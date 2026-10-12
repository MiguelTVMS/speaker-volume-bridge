//! macOS Core Audio adapter using a focused FFI surface.
//!
//! Core Audio invokes property callbacks on implementation-controlled threads.
//! Each callback only reads the changed state and sends a bounded event; it never
//! waits for synchronization or network work.

#![allow(
    clippy::borrow_as_ptr,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_ptr_alignment,
    clippy::needless_pass_by_value,
    clippy::semicolon_if_nothing_returned
)]

use crate::expected_writes::ExpectedWrites;
use crate::{
    AudioDeviceSelection, AudioOutputDevice, PlatformAudioError, SystemAudioController,
    SystemAudioEvent,
};
use async_trait::async_trait;
use speaker_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, NormalizedVolume};
use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, oneshot};

type AudioObjectID = u32;
type OSStatus = i32;
type UInt32 = u32;
type Boolean = u8;

const NO_ERR: OSStatus = 0;
const SYSTEM_OBJECT: AudioObjectID = 1;
const SCOPE_GLOBAL: UInt32 = 0x676c_6f62; // 'glob'
const SCOPE_OUTPUT: UInt32 = 0x6f75_7470; // 'outp'
const ELEMENT_MASTER: UInt32 = 0;
const DEFAULT_OUTPUT_DEVICE: UInt32 = 0x644f_7574; // 'dOut'
const DEVICES: UInt32 = 0x6465_7623; // 'dev#'
const OBJECT_NAME: UInt32 = 0x6c6e_616d; // 'lnam'
const DEVICE_UID: UInt32 = 0x7569_6420; // 'uid '
const VOLUME_SCALAR: UInt32 = 0x766f_6c6d; // 'volm'
const MUTE: UInt32 = 0x6d75_7465; // 'mute'
const STREAM_CONFIGURATION: UInt32 = 0x736c_6179; // 'slay'
const DEVICE_UNAVAILABLE: OSStatus = -2;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioObjectPropertyAddress {
    selector: UInt32,
    scope: UInt32,
    element: UInt32,
}

type Listener = unsafe extern "C" fn(
    AudioObjectID,
    UInt32,
    *const AudioObjectPropertyAddress,
    *mut c_void,
) -> OSStatus;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectHasProperty(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
    ) -> Boolean;
    fn AudioObjectIsPropertySettable(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        settable: *mut Boolean,
    ) -> OSStatus;
    fn AudioObjectGetPropertyData(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: UInt32,
        qualifier: *const c_void,
        data_size: *mut UInt32,
        data: *mut c_void,
    ) -> OSStatus;
    fn AudioObjectGetPropertyDataSize(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: UInt32,
        qualifier: *const c_void,
        data_size: *mut UInt32,
    ) -> OSStatus;
    fn AudioObjectSetPropertyData(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: UInt32,
        qualifier: *const c_void,
        data_size: UInt32,
        data: *const c_void,
    ) -> OSStatus;
    fn AudioObjectAddPropertyListener(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        listener: Listener,
        client_data: *mut c_void,
    ) -> OSStatus;
    fn AudioObjectRemovePropertyListener(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        listener: Listener,
        client_data: *mut c_void,
    ) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringGetCString(
        string: *const c_void,
        buffer: *mut i8,
        size: isize,
        encoding: u32,
    ) -> Boolean;
}

const UTF8: u32 = 0x0800_0100;

/// Lists physical or virtual devices that expose output streams.
pub fn list_output_devices() -> Result<Vec<AudioOutputDevice>, PlatformAudioError> {
    // SAFETY: all FFI calls use correctly sized buffers and only copy Core Audio-owned values.
    unsafe { output_devices().map_err(map_error) }
}

/// Converts a legacy numeric Core Audio object ID to its persistent device UID.
///
/// Core Audio object IDs may change when a USB output reconnects. This is only
/// used to migrate existing settings while that output is still available.
pub fn migrate_legacy_device_id(id: &str) -> Option<String> {
    let id = legacy_audio_object_id(id)?;
    // SAFETY: this only reads the UID property from the currently registered device.
    unsafe { device_uid(id) }
}

#[derive(Clone)]
pub struct MacosAudioController {
    worker: Arc<WorkerHandle>,
    events: broadcast::Sender<SystemAudioEvent>,
}

struct WorkerHandle {
    commands: SyncSender<Command>,
    shutdown: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let thread = self
            .thread
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl MacosAudioController {
    pub fn start(
        selection: AudioDeviceSelection,
        tolerance: u8,
    ) -> Result<Self, PlatformAudioError> {
        let (commands, receiver) = mpsc::sync_channel(32);
        let (events, _) = broadcast::channel(64);
        let worker_events = events.clone();
        let worker_commands = commands.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("speaker-volume-bridge-core-audio".to_owned())
            .spawn(move || {
                run_worker(
                    selection,
                    tolerance,
                    receiver,
                    worker_commands,
                    worker_events,
                    &worker_shutdown,
                )
            })
            .map_err(|error| PlatformAudioError::Platform(error.to_string()))?;
        Ok(Self {
            worker: Arc::new(WorkerHandle {
                commands,
                shutdown,
                thread: Mutex::new(Some(thread)),
            }),
            events,
        })
    }

    async fn request<T>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<T, PlatformAudioError>>) -> Command,
    ) -> Result<T, PlatformAudioError> {
        let (response, receiver) = oneshot::channel();
        self.worker
            .commands
            .try_send(command(response))
            .map_err(|_| PlatformAudioError::DeviceUnavailable)?;
        receiver
            .await
            .map_err(|_| PlatformAudioError::DeviceUnavailable)?
    }
}

#[async_trait]
impl SystemAudioController for MacosAudioController {
    async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> {
        self.request(Command::Current).await
    }
    async fn set_volume(
        &self,
        volume: NormalizedVolume,
        _: LocalOrigin,
    ) -> Result<(), PlatformAudioError> {
        self.request(|response| Command::SetVolume { volume, response })
            .await
    }
    async fn set_muted(&self, muted: bool, _: LocalOrigin) -> Result<(), PlatformAudioError> {
        self.request(|response| Command::SetMuted { muted, response })
            .await
    }
    fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> {
        self.events.subscribe()
    }
}

enum Command {
    Current(oneshot::Sender<Result<LocalAudioState, PlatformAudioError>>),
    SetVolume {
        volume: NormalizedVolume,
        response: oneshot::Sender<Result<(), PlatformAudioError>>,
    },
    SetMuted {
        muted: bool,
        response: oneshot::Sender<Result<(), PlatformAudioError>>,
    },
    Reattach,
}

struct CallbackContext {
    events: broadcast::Sender<SystemAudioEvent>,
    commands: SyncSender<Command>,
    expected: Mutex<ExpectedWrites>,
    tolerance: u8,
}

fn run_worker(
    selection: AudioDeviceSelection,
    tolerance: u8,
    receiver: Receiver<Command>,
    commands: SyncSender<Command>,
    events: broadcast::Sender<SystemAudioEvent>,
    shutdown: &AtomicBool,
) {
    let context = Arc::new(CallbackContext {
        events: events.clone(),
        commands,
        expected: Mutex::new(ExpectedWrites::default()),
        tolerance,
    });
    let mut endpoint = match unsafe { Endpoint::attach(&selection, Arc::clone(&context)) } {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("Core Audio selected output attachment failed with OSStatus {error}");
            let _ = events.send(SystemAudioEvent::DeviceUnavailable { device_id: None });
            return;
        }
    };
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let command = match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match command {
            Command::Current(response) => {
                let _ = response.send(unsafe { endpoint.state() }.map_err(map_error));
            }
            Command::SetVolume { volume, response } => {
                let _ = response.send(unsafe { endpoint.set_volume(volume) }.map_err(map_error));
            }
            Command::SetMuted { muted, response } => {
                let _ = response.send(unsafe { endpoint.set_muted(muted) });
            }
            Command::Reattach => {
                match unsafe { endpoint.refresh(&selection, Arc::clone(&context)) } {
                    Ok(false) => {} // A notification about an unrelated output needs no work.
                    Ok(true) => {
                        if let Ok(mut expected) = context.expected.lock() {
                            *expected = ExpectedWrites::default();
                        }
                        let _ = events.send(SystemAudioEvent::DefaultOutputChanged);
                    }
                    Err(error) => {
                        eprintln!(
                            "Core Audio selected output reattachment failed with OSStatus {error}"
                        );
                        let _ =
                            events.send(SystemAudioEvent::DeviceUnavailable { device_id: None });
                        break;
                    }
                }
            }
        }
    }
    endpoint.detach();
}

struct Endpoint {
    id: AudioObjectID,
    channels: Vec<UInt32>,
    has_mute: bool,
    context: Arc<CallbackContext>,
    listeners: ListenerRegistrations,
}

impl Endpoint {
    unsafe fn attach(
        selection: &AudioDeviceSelection,
        context: Arc<CallbackContext>,
    ) -> Result<Self, OSStatus> {
        let (id, channels, has_mute) = unsafe { resolve_endpoint(selection)? };
        let mut endpoint = Self {
            id,
            channels,
            has_mute,
            context,
            listeners: ListenerRegistrations::default(),
        };
        unsafe {
            endpoint.listen()?;
        }
        Ok(endpoint)
    }
    unsafe fn refresh(
        &mut self,
        selection: &AudioDeviceSelection,
        context: Arc<CallbackContext>,
    ) -> Result<bool, OSStatus> {
        let (id, channels, has_mute) = unsafe { resolve_endpoint(selection)? };
        let unchanged = self.id == id && self.channels == channels && self.has_mute == has_mute;
        refresh_endpoint(
            self,
            unchanged,
            |endpoint| unsafe { endpoint.state().map(|_| ()) },
            Self::detach,
            || unsafe { Self::attach(selection, context) },
        )
    }
    unsafe fn state(&self) -> Result<LocalAudioState, OSStatus> {
        let mut total = 0.0_f32;
        for channel in &self.channels {
            total += unsafe { scalar(self.id, address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel))? };
        }
        let volume = normalize(total / self.channels.len() as f32);
        let muted = if self.has_mute {
            unsafe { boolean(self.id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER))? }
        } else {
            false
        };
        Ok(LocalAudioState {
            volume,
            muted: MuteState(muted),
        })
    }
    unsafe fn set_volume(&self, volume: NormalizedVolume) -> Result<(), OSStatus> {
        let muted = unsafe { self.state()? }.muted;
        let value = f32::from(volume.get()) / 100.0;
        let channels = self
            .channels
            .iter()
            .map(|channel| unsafe {
                scalar(self.id, address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Per-channel devices can notify after each setter, before the final average.
        for intermediate in channel_write_volumes(channels, value) {
            self.expect(LocalAudioState {
                volume: intermediate,
                muted,
            });
        }
        for channel in &self.channels {
            unsafe {
                set_scalar(
                    self.id,
                    address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel),
                    value,
                )?;
            }
        }
        Ok(())
    }
    unsafe fn set_muted(&self, muted: bool) -> Result<(), PlatformAudioError> {
        if !self.has_mute {
            return Err(PlatformAudioError::MuteUnavailable);
        }
        let state = LocalAudioState {
            volume: unsafe { self.state().map_err(map_error)? }.volume,
            muted: MuteState(muted),
        };
        self.expect(state);
        unsafe { set_boolean(self.id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER), muted) }
            .map_err(map_error)
    }
    fn expect(&self, state: LocalAudioState) {
        if let Ok(mut expected) = self.context.expected.lock() {
            expected.record(state, Instant::now());
        }
    }

    unsafe fn listen(&mut self) -> Result<(), OSStatus> {
        let data = Arc::as_ptr(&self.context).cast_mut().cast::<c_void>();
        let mut registrations = self
            .channels
            .iter()
            .map(|channel| ListenerRegistration {
                object: self.id,
                address: address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel),
                callback: property_changed,
            })
            .collect::<Vec<_>>();
        if self.has_mute {
            registrations.push(ListenerRegistration {
                object: self.id,
                address: address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER),
                callback: property_changed,
            });
        }
        registrations.extend([
            ListenerRegistration {
                object: SYSTEM_OBJECT,
                address: address(DEFAULT_OUTPUT_DEVICE, SCOPE_GLOBAL, ELEMENT_MASTER),
                callback: default_output_changed,
            },
            ListenerRegistration {
                object: SYSTEM_OBJECT,
                address: address(DEVICES, SCOPE_GLOBAL, ELEMENT_MASTER),
                callback: device_list_changed,
            },
        ]);
        self.listeners.register(
            registrations,
            |registration| unsafe {
                AudioObjectAddPropertyListener(
                    registration.object,
                    &registration.address,
                    registration.callback,
                    data,
                )
            },
            |registration| unsafe {
                AudioObjectRemovePropertyListener(
                    registration.object,
                    &registration.address,
                    registration.callback,
                    data,
                )
            },
        )
    }
    fn detach(&mut self) {
        let data = Arc::as_ptr(&self.context).cast_mut().cast::<c_void>();
        // SAFETY: only successfully installed listeners are removed, while context remains alive.
        self.listeners.remove(|registration| unsafe {
            AudioObjectRemovePropertyListener(
                registration.object,
                &registration.address,
                registration.callback,
                data,
            )
        });
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.detach();
    }
}

unsafe fn resolve_endpoint(
    selection: &AudioDeviceSelection,
) -> Result<(AudioObjectID, Vec<UInt32>, bool), OSStatus> {
    let id = match selection {
        AudioDeviceSelection::FollowDefault => unsafe { default_output_device()? },
        AudioDeviceSelection::Fixed { device_id } => unsafe { resolve_fixed_output(device_id)? },
    };
    let master = address(VOLUME_SCALAR, SCOPE_OUTPUT, ELEMENT_MASTER);
    let has_master = unsafe { AudioObjectHasProperty(id, &master) != 0 }
        && matches!(unsafe { settable(id, &master) }, Ok(true));
    let channels = if has_master {
        vec![ELEMENT_MASTER]
    } else {
        unsafe { output_channels(id)? }
    };
    if channels.is_empty() {
        return Err(-1);
    }
    let mute_address = address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER);
    let has_mute = unsafe { AudioObjectHasProperty(id, &mute_address) != 0 }
        && matches!(unsafe { settable(id, &mute_address) }, Ok(true));
    Ok((id, channels, has_mute))
}

// Healthy, unchanged outputs keep their listeners and current synchronization session.
fn refresh_endpoint<T, E>(
    current: &mut T,
    unchanged: bool,
    verify: impl FnOnce(&T) -> Result<(), E>,
    detach: impl FnOnce(&mut T),
    attach: impl FnOnce() -> Result<T, E>,
) -> Result<bool, E> {
    if unchanged {
        verify(current)?;
        return Ok(false);
    }
    replace_endpoint(current, detach, attach)?;
    Ok(true)
}

// Core Audio rejects a duplicate callback/address/context registration. Remove the
// old set before attachment, including when reselecting the same physical output.
fn replace_endpoint<T, E>(
    current: &mut T,
    detach: impl FnOnce(&mut T),
    attach: impl FnOnce() -> Result<T, E>,
) -> Result<(), E> {
    detach(current);
    *current = attach()?;
    Ok(())
}

#[derive(Clone, Copy)]
struct ListenerRegistration {
    object: AudioObjectID,
    address: AudioObjectPropertyAddress,
    callback: Listener,
}

#[derive(Default)]
struct ListenerRegistrations(Vec<ListenerRegistration>);

impl ListenerRegistrations {
    fn register(
        &mut self,
        registrations: impl IntoIterator<Item = ListenerRegistration>,
        mut add: impl FnMut(&ListenerRegistration) -> OSStatus,
        mut remove: impl FnMut(&ListenerRegistration) -> OSStatus,
    ) -> Result<(), OSStatus> {
        for registration in registrations {
            let status = add(&registration);
            if status != NO_ERR {
                self.remove(&mut remove);
                return Err(status);
            }
            self.0.push(registration);
        }
        Ok(())
    }

    fn remove(&mut self, mut remove: impl FnMut(&ListenerRegistration) -> OSStatus) {
        for registration in self.0.drain(..).rev() {
            let _ = remove(&registration);
        }
    }
}

fn channel_write_volumes(mut channels: Vec<f32>, target: f32) -> Vec<NormalizedVolume> {
    let mut volumes = Vec::with_capacity(channels.len());
    for index in 0..channels.len() {
        channels[index] = target;
        volumes.push(normalize(
            channels.iter().sum::<f32>() / channels.len() as f32,
        ));
    }
    volumes
}

unsafe extern "C" fn property_changed(
    object: AudioObjectID,
    _: UInt32,
    _: *const AudioObjectPropertyAddress,
    data: *mut c_void,
) -> OSStatus {
    let Some(context) = (unsafe { data.cast::<CallbackContext>().as_ref() }) else {
        return -1;
    };
    let state = unsafe { state_for_callback(object) };
    if let Ok(state) = state {
        let origin =
            if context.expected.lock().is_ok_and(|mut expected| {
                expected.matches(state, context.tolerance, Instant::now())
            }) {
                LocalOrigin::Application
            } else {
                LocalOrigin::User
            };
        let _ = context
            .events
            .send(SystemAudioEvent::StateChanged { state, origin });
    }
    NO_ERR
}

unsafe extern "C" fn default_output_changed(
    _: AudioObjectID,
    _: UInt32,
    _: *const AudioObjectPropertyAddress,
    data: *mut c_void,
) -> OSStatus {
    if let Some(context) = unsafe { data.cast::<CallbackContext>().as_ref() } {
        let _ = context.commands.try_send(Command::Reattach);
    }
    NO_ERR
}

unsafe extern "C" fn device_list_changed(
    _: AudioObjectID,
    _: UInt32,
    _: *const AudioObjectPropertyAddress,
    data: *mut c_void,
) -> OSStatus {
    if let Some(context) = unsafe { data.cast::<CallbackContext>().as_ref() } {
        let _ = context.commands.try_send(Command::Reattach);
    }
    NO_ERR
}

unsafe fn state_for_callback(id: AudioObjectID) -> Result<LocalAudioState, OSStatus> {
    let master = address(VOLUME_SCALAR, SCOPE_OUTPUT, ELEMENT_MASTER);
    if unsafe { AudioObjectHasProperty(id, &master) != 0 } {
        return Ok(LocalAudioState {
            volume: normalize(unsafe { scalar(id, master)? }),
            muted: MuteState(unsafe {
                boolean_if_present(id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER))?
            }),
        });
    }
    let channels = unsafe { output_channels(id)? };
    if channels.is_empty() {
        return Err(-1);
    }
    let total = channels.iter().try_fold(0.0_f32, |total, channel| unsafe {
        scalar(id, address(VOLUME_SCALAR, SCOPE_OUTPUT, *channel)).map(|value| total + value)
    })?;
    Ok(LocalAudioState {
        volume: normalize(total / channels.len() as f32),
        muted: MuteState(unsafe {
            boolean_if_present(id, address(MUTE, SCOPE_OUTPUT, ELEMENT_MASTER))?
        }),
    })
}

fn address(selector: UInt32, scope: UInt32, element: UInt32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        selector,
        scope,
        element,
    }
}
unsafe fn default_output_device() -> Result<AudioObjectID, OSStatus> {
    unsafe {
        get(
            SYSTEM_OBJECT,
            address(DEFAULT_OUTPUT_DEVICE, SCOPE_GLOBAL, ELEMENT_MASTER),
        )
    }
}
unsafe fn resolve_fixed_output(device_id: &str) -> Result<AudioObjectID, OSStatus> {
    if let Some(id) = legacy_audio_object_id(device_id) {
        return Ok(id);
    }
    let devices = unsafe { audio_object_ids()? };
    devices
        .into_iter()
        .find(|id| unsafe { device_uid(*id).as_deref() == Some(device_id) })
        .ok_or(DEVICE_UNAVAILABLE)
}
unsafe fn scalar(id: AudioObjectID, address: AudioObjectPropertyAddress) -> Result<f32, OSStatus> {
    unsafe { get(id, address) }
}
unsafe fn boolean(
    id: AudioObjectID,
    address: AudioObjectPropertyAddress,
) -> Result<bool, OSStatus> {
    Ok(unsafe { get::<u32>(id, address)? } != 0)
}
unsafe fn boolean_if_present(
    id: AudioObjectID,
    address: AudioObjectPropertyAddress,
) -> Result<bool, OSStatus> {
    if unsafe { AudioObjectHasProperty(id, &address) != 0 } {
        unsafe { boolean(id, address) }
    } else {
        Ok(false)
    }
}
unsafe fn set_scalar(
    id: AudioObjectID,
    address: AudioObjectPropertyAddress,
    value: f32,
) -> Result<(), OSStatus> {
    unsafe { set(id, address, &value) }
}
unsafe fn set_boolean(
    id: AudioObjectID,
    address: AudioObjectPropertyAddress,
    value: bool,
) -> Result<(), OSStatus> {
    unsafe { set(id, address, &(u32::from(value))) }
}
unsafe fn get<T: Copy>(
    id: AudioObjectID,
    address: AudioObjectPropertyAddress,
) -> Result<T, OSStatus> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut size = std::mem::size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            value.as_mut_ptr().cast(),
        )
    };
    if status == NO_ERR && size == std::mem::size_of::<T>() as u32 {
        Ok(unsafe { value.assume_init() })
    } else {
        Err(status)
    }
}
unsafe fn set<T>(
    id: AudioObjectID,
    address: AudioObjectPropertyAddress,
    value: &T,
) -> Result<(), OSStatus> {
    let status = unsafe {
        AudioObjectSetPropertyData(
            id,
            &address,
            0,
            std::ptr::null(),
            std::mem::size_of::<T>() as u32,
            std::ptr::from_ref(value).cast(),
        )
    };
    if status == NO_ERR {
        Ok(())
    } else {
        Err(status)
    }
}
unsafe fn settable(
    id: AudioObjectID,
    address: &AudioObjectPropertyAddress,
) -> Result<bool, OSStatus> {
    let mut settable = 0;
    let status = unsafe { AudioObjectIsPropertySettable(id, address, &mut settable) };
    if status == NO_ERR {
        Ok(settable != 0)
    } else {
        Err(status)
    }
}
unsafe fn output_channels(id: AudioObjectID) -> Result<Vec<UInt32>, OSStatus> {
    let stream_address = address(STREAM_CONFIGURATION, SCOPE_OUTPUT, ELEMENT_MASTER);
    let mut size = 0_u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(id, &stream_address, 0, std::ptr::null(), &mut size)
    };
    if status != NO_ERR || size < 8 {
        return Err(status);
    }
    // AudioBufferList starts with `mNumberBuffers`; each buffer contributes its channel count.
    let mut bytes = vec![0_u8; size as usize];
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &stream_address,
            0,
            std::ptr::null(),
            &mut size,
            bytes.as_mut_ptr().cast(),
        )
    };
    if status != NO_ERR {
        return Err(status);
    }
    let buffers = unsafe { *(bytes.as_ptr().cast::<u32>()) } as usize;
    let mut offset = 8_usize;
    let mut channels = Vec::new();
    for _ in 0..buffers {
        if offset + 16 > bytes.len() {
            return Err(-1);
        }
        let count = unsafe { *(bytes.as_ptr().add(offset).cast::<u32>()) };
        for channel in 1..=count {
            let volume_address = address(VOLUME_SCALAR, SCOPE_OUTPUT, channel);
            if unsafe {
                AudioObjectHasProperty(id, &volume_address) != 0
                    && matches!(settable(id, &volume_address), Ok(true))
            } {
                channels.push(channel);
            }
        }
        offset += 16;
    }
    Ok(channels)
}

unsafe fn output_devices() -> Result<Vec<AudioOutputDevice>, OSStatus> {
    let ids = unsafe { audio_object_ids()? };
    Ok(ids
        .into_iter()
        .filter_map(|id| unsafe { output_device(id) })
        .collect())
}

unsafe fn audio_object_ids() -> Result<Vec<AudioObjectID>, OSStatus> {
    let devices_address = address(DEVICES, SCOPE_GLOBAL, ELEMENT_MASTER);
    let mut size = 0_u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            SYSTEM_OBJECT,
            &devices_address,
            0,
            std::ptr::null(),
            &mut size,
        )
    };
    if status != NO_ERR || !size.is_multiple_of(std::mem::size_of::<AudioObjectID>() as u32) {
        return Err(status);
    }
    let mut ids = vec![0_u32; size as usize / std::mem::size_of::<AudioObjectID>()];
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &devices_address,
            0,
            std::ptr::null(),
            &mut size,
            ids.as_mut_ptr().cast(),
        )
    };
    if status != NO_ERR {
        return Err(status);
    }
    Ok(ids)
}

unsafe fn output_device(id: AudioObjectID) -> Option<AudioOutputDevice> {
    let stream = address(STREAM_CONFIGURATION, SCOPE_OUTPUT, ELEMENT_MASTER);
    if unsafe { AudioObjectHasProperty(id, &stream) == 0 } {
        return None;
    }
    let master = address(VOLUME_SCALAR, SCOPE_OUTPUT, ELEMENT_MASTER);
    let writable_volume = (unsafe { AudioObjectHasProperty(id, &master) != 0 }
        && matches!(unsafe { settable(id, &master) }, Ok(true)))
        || unsafe { output_channels(id).is_ok_and(|channels| !channels.is_empty()) };
    Some(AudioOutputDevice {
        id: unsafe { device_uid(id) }?,
        name: unsafe { device_name(id) }.unwrap_or_else(|| format!("Output device {id}")),
        writable_volume,
    })
}

unsafe fn device_name(id: AudioObjectID) -> Option<String> {
    unsafe { string_property(id, OBJECT_NAME) }
}
unsafe fn device_uid(id: AudioObjectID) -> Option<String> {
    unsafe { string_property(id, DEVICE_UID) }
}
unsafe fn string_property(id: AudioObjectID, selector: UInt32) -> Option<String> {
    let mut value: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of_val(&value) as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &address(selector, SCOPE_GLOBAL, ELEMENT_MASTER),
            0,
            std::ptr::null(),
            &mut size,
            std::ptr::from_mut(&mut value).cast(),
        )
    };
    if status != NO_ERR || value.is_null() {
        return None;
    }
    let mut bytes = [0_i8; 512];
    if unsafe { CFStringGetCString(value, bytes.as_mut_ptr(), 512, UTF8) } == 0 {
        return None;
    }
    std::ffi::CStr::from_bytes_until_nul(unsafe {
        std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len())
    })
    .ok()?
    .to_str()
    .ok()
    .map(str::to_owned)
}
fn map_error(status: OSStatus) -> PlatformAudioError {
    if status == DEVICE_UNAVAILABLE {
        PlatformAudioError::DeviceUnavailable
    } else if status == -1 {
        PlatformAudioError::UnsupportedDevice
    } else {
        PlatformAudioError::Platform(format!("Core Audio OSStatus {status}"))
    }
}
fn legacy_audio_object_id(id: &str) -> Option<AudioObjectID> {
    id.parse().ok()
}
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn normalize(value: f32) -> NormalizedVolume {
    NormalizedVolume::new((value.clamp(0.0, 1.0) * 100.0).round() as u8)
        .unwrap_or(NormalizedVolume::MIN)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    #[test]
    #[ignore = "requires a writable native Core Audio output; reads and registers listeners only"]
    fn native_same_output_reattachment_retains_registered_listeners() {
        let selection = std::env::var("SVB_NATIVE_AUDIO_DEVICE")
            .map_or(super::AudioDeviceSelection::FollowDefault, |device_id| {
                super::AudioDeviceSelection::Fixed { device_id }
            });
        let (events, _) = tokio::sync::broadcast::channel(64);
        let (commands, _receiver) = std::sync::mpsc::sync_channel(32);
        let context = std::sync::Arc::new(super::CallbackContext {
            events,
            commands,
            expected: std::sync::Mutex::new(super::ExpectedWrites::default()),
            tolerance: 1,
        });
        let mut endpoint =
            unsafe { super::Endpoint::attach(&selection, std::sync::Arc::clone(&context)) }
                .unwrap();
        for _ in 0..3 {
            super::replace_endpoint(&mut endpoint, super::Endpoint::detach, || unsafe {
                super::Endpoint::attach(&selection, std::sync::Arc::clone(&context))
            })
            .unwrap();
            assert!(unsafe { endpoint.state() }.is_ok());
            let data = std::sync::Arc::as_ptr(&context)
                .cast_mut()
                .cast::<std::ffi::c_void>();
            // A duplicate must be rejected, proving the replacement remains registered.
            for registration in &endpoint.listeners.0 {
                assert_eq!(
                    unsafe {
                        super::AudioObjectAddPropertyListener(
                            registration.object,
                            &registration.address,
                            registration.callback,
                            data,
                        )
                    },
                    1_852_797_029
                );
            }
        }
    }

    #[test]
    fn unchanged_output_notification_does_not_replace_listeners_or_hide_read_failure() {
        let mut endpoint = 2;
        let changed = super::refresh_endpoint(
            &mut endpoint,
            true,
            |_| Ok::<_, i32>(()),
            |_| panic!("must retain healthy listeners"),
            || panic!("must not reattach unchanged output"),
        )
        .unwrap();
        assert!(!changed);
        assert_eq!(endpoint, 2);
        assert_eq!(
            super::refresh_endpoint(
                &mut endpoint,
                true,
                |_| Err(7),
                |_| panic!("no attachment on failed verification"),
                || Ok(3)
            ),
            Err(7)
        );
        assert!(
            super::refresh_endpoint(
                &mut endpoint,
                false,
                |_| panic!("changed output"),
                |old| *old = 0,
                || Ok::<_, i32>(3)
            )
            .unwrap()
        );
        assert_eq!(endpoint, 3);
    }

    #[tokio::test]
    #[ignore = "requires a writable native output; sends read-only topology notifications"]
    async fn native_unchanged_output_notifications_keep_session() {
        use super::SystemAudioController;
        let selection = std::env::var("SVB_NATIVE_AUDIO_DEVICE")
            .map_or(super::AudioDeviceSelection::FollowDefault, |device_id| {
                super::AudioDeviceSelection::Fixed { device_id }
            });
        let controller = super::MacosAudioController::start(selection, 1).unwrap();
        let mut events = controller.subscribe();
        controller.current_state().await.unwrap();
        for _ in 0..5 {
            controller
                .worker
                .commands
                .send(super::Command::Reattach)
                .unwrap();
            // The read is a FIFO barrier after the real worker processes the notification.
            controller.current_state().await.unwrap();
            while let Ok(event) = events.try_recv() {
                assert!(
                    matches!(event, super::SystemAudioEvent::StateChanged { .. }),
                    "unchanged output must not report a lifecycle change: {event:?}"
                );
            }
        }
    }

    fn test_registrations() -> Vec<super::ListenerRegistration> {
        [
            (2, super::VOLUME_SCALAR, super::SCOPE_OUTPUT, 1),
            (2, super::VOLUME_SCALAR, super::SCOPE_OUTPUT, 2),
            (2, super::MUTE, super::SCOPE_OUTPUT, 0),
            (
                super::SYSTEM_OBJECT,
                super::DEFAULT_OUTPUT_DEVICE,
                super::SCOPE_GLOBAL,
                0,
            ),
            (super::SYSTEM_OBJECT, super::DEVICES, super::SCOPE_GLOBAL, 0),
        ]
        .into_iter()
        .map(
            |(object, selector, scope, element)| super::ListenerRegistration {
                object,
                address: super::address(selector, scope, element),
                callback: super::property_changed,
            },
        )
        .collect()
    }

    fn listener_key(registration: &super::ListenerRegistration) -> (u32, u32, u32, u32) {
        (
            registration.object,
            registration.address.selector,
            registration.address.scope,
            registration.address.element,
        )
    }

    #[test]
    fn same_output_reattachment_removes_old_listeners_before_registering_replacements() {
        let active = RefCell::new(BTreeSet::new());
        let add = |registration: &super::ListenerRegistration| {
            if active.borrow_mut().insert(listener_key(registration)) {
                0
            } else {
                1_852_797_029
            }
        };
        let remove = |registration: &super::ListenerRegistration| {
            assert!(active.borrow_mut().remove(&listener_key(registration)));
            0
        };
        let attach = || {
            let mut registrations = super::ListenerRegistrations::default();
            registrations.register(test_registrations(), add, remove)?;
            Ok::<_, i32>(registrations)
        };
        let mut endpoint = attach().unwrap();
        // Startup followed by repeated device/default-output notifications for the same output.
        for _ in 0..3 {
            super::replace_endpoint(&mut endpoint, |old| old.remove(remove), attach).unwrap();
            assert_eq!(active.borrow().len(), 5);
        }
        endpoint.remove(remove);
        endpoint.remove(remove);
        assert!(active.borrow().is_empty());
    }

    #[test]
    fn failed_listener_attachment_rolls_back_only_successful_registrations() {
        for failure_index in 0..5 {
            let active = RefCell::new(BTreeSet::new());
            let mut attempt = 0;
            let mut registrations = super::ListenerRegistrations::default();
            let result = registrations.register(
                test_registrations(),
                |registration| {
                    let fail = attempt == failure_index;
                    attempt += 1;
                    if fail {
                        return 1_852_797_029;
                    }
                    assert!(active.borrow_mut().insert(listener_key(registration)));
                    0
                },
                |registration| {
                    assert!(active.borrow_mut().remove(&listener_key(registration)));
                    0
                },
            );
            assert_eq!(result, Err(1_852_797_029));
            assert!(active.borrow().is_empty());
            assert!(registrations.0.is_empty());
        }
    }

    use super::*;
    use speaker_volume_bridge_domain::ExpectedLocalWrite;
    #[test]
    fn channel_transitions_and_duplicate_final_callbacks_are_suppressed() {
        let now = Instant::now();
        let mut expected = ExpectedWrites::default();
        let volumes = channel_write_volumes(vec![0.2, 0.4], 0.6);
        assert_eq!(
            volumes
                .iter()
                .map(|volume| volume.get())
                .collect::<Vec<_>>(),
            vec![50, 60]
        );
        for volume in volumes {
            expected.record(
                LocalAudioState {
                    volume,
                    muted: MuteState(false),
                },
                now,
            );
        }
        for volume in [50, 60, 50, 60] {
            assert!(expected.matches(
                LocalAudioState {
                    volume: NormalizedVolume::new(volume).unwrap(),
                    muted: MuteState(false),
                },
                1,
                now
            ));
        }
    }
    #[test]
    fn expected_writes_suppress_matching_callbacks() {
        let expected = ExpectedLocalWrite {
            state: LocalAudioState {
                volume: NormalizedVolume::new(42).unwrap(),
                muted: MuteState(false),
            },
            expires_at_ms: 10,
            generation: 1,
            tolerance: 1,
        };
        let observed = LocalAudioState {
            volume: NormalizedVolume::new(43).unwrap(),
            muted: MuteState(false),
        };
        assert_eq!(
            expected.classify(observed, 9),
            speaker_volume_bridge_domain::SuppressionDecision::Suppress
        );
    }

    #[test]
    fn recognizes_legacy_numeric_device_ids_only() {
        assert_eq!(legacy_audio_object_id("331"), Some(331));
        assert_eq!(legacy_audio_object_id("com.example.output"), None);
    }
}
