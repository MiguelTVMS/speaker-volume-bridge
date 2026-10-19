//! Media Foundation activity reports only; enumeration never activates a camera.
#![allow(unsafe_code, non_snake_case)] // Windows COM ABI.
use super::{
    inventory::Inventory,
    query::{self, QuerySource},
    worker::{Monitor, unavailable},
};
use speaker_volume_bridge_integration::camera::CameraPort;
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
use windows::Win32::{
    Media::MediaFoundation::{
        IMFActivate, IMFSensorActivitiesReport, IMFSensorActivitiesReportCallback,
        IMFSensorActivitiesReportCallback_Impl, IMFShutdown, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION, MFCreateAttributes,
        MFCreateSensorActivityMonitor, MFEnumDeviceSources, MFSTARTUP_LITE, MFShutdown, MFStartup,
    },
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
};
use windows_core::{Interface, Ref};
type Cameras = Arc<Mutex<Inventory<Vec<u16>>>>;
fn failure() -> windows_core::Error {
    windows_core::Error::from_hresult(windows_core::HRESULT(0x8000_4005_u32.cast_signed()))
}

// Scope lint exceptions to COM macro output, not the native adapter implementation.
#[allow(clippy::ref_as_ptr, clippy::inline_always)]
mod callback {
    use super::{
        Cameras, IMFSensorActivitiesReport, IMFSensorActivitiesReportCallback,
        IMFSensorActivitiesReportCallback_Impl, Ref, failure,
    };
    use windows_core::implement;
    #[implement(IMFSensorActivitiesReportCallback)]
    pub(super) struct Callback {
        pub(super) cameras: Cameras,
    }
    impl IMFSensorActivitiesReportCallback_Impl for Callback_Impl {
        fn OnActivitiesReport(
            &self,
            report: Ref<'_, IMFSensorActivitiesReport>,
        ) -> windows_core::Result<()> {
            let result = (|| {
                let report = report.ok()?;
                let mut cameras = self.cameras.lock().map_err(|_| failure())?;
                // SAFETY: COM owns all interfaces during these calls. No process identity is queried.
                unsafe {
                    for index in 0..report.GetCount()? {
                        let device = report.GetActivityReport(index)?;
                        let mut key = vec![0_u16; 4096];
                        let mut used = 0;
                        device.GetSymbolicLink(&mut key, &raw mut used)?;
                        key.truncate(key.iter().position(|c| *c == 0).ok_or_else(failure)?);
                        let mut active = false;
                        for process in 0..device.GetProcessCount()? {
                            active |= device
                                .GetProcessActivity(process)?
                                .GetStreamingState()?
                                .as_bool();
                        }
                        cameras.update(&key, active);
                    }
                }
                Ok::<_, windows_core::Error>(())
            })();
            if result.is_err()
                && let Ok(mut cameras) = self.cameras.lock()
            {
                cameras.fail();
            }
            Ok(())
        }
    }
}
use callback::Callback;

struct Sources {
    pointer: *mut Option<IMFActivate>,
    count: u32,
}
impl Drop for Sources {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            // SAFETY: MFEnumDeviceSources allocates count initialized COM references and a task-memory array.
            unsafe {
                for index in 0..self.count as usize {
                    std::ptr::drop_in_place(self.pointer.add(index));
                }
                CoTaskMemFree(Some(self.pointer.cast()));
            }
        }
    }
}
fn camera_keys() -> windows_core::Result<Vec<Vec<u16>>> {
    // SAFETY: only enumerate activation metadata; never call ActivateObject.
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&raw mut attributes, 1)?;
        let attributes = attributes.ok_or_else(failure)?;
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
        let mut sources = Sources {
            pointer: std::ptr::null_mut(),
            count: 0,
        };
        MFEnumDeviceSources(
            &attributes,
            &raw mut sources.pointer,
            &raw mut sources.count,
        )?;
        if sources.count != 0 && sources.pointer.is_null() {
            return Err(failure());
        }
        let mut keys = Vec::new();
        for index in 0..sources.count as usize {
            let device = (*sources.pointer.add(index)).as_ref().ok_or_else(failure)?;
            let length =
                device.GetStringLength(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)?;
            if length > 65535 {
                return Err(failure());
            }
            let mut key = vec![0_u16; length as usize + 1];
            device.GetString(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                &mut key,
                None,
            )?;
            key.truncate(length as usize);
            keys.push(key);
        }
        Ok(keys)
    }
}

struct NativeQuery<'a> {
    monitor: Option<windows::Win32::Media::MediaFoundation::IMFSensorActivityMonitor>,
    stopped: &'a mpsc::Receiver<()>,
    stopping: bool,
}
impl QuerySource for NativeQuery<'_> {
    type Key = Vec<u16>;
    fn devices(&mut self) -> Option<Vec<Self::Key>> {
        camera_keys().ok()
    }
    fn start(&mut self, cameras: Cameras) -> bool {
        let callback: IMFSensorActivitiesReportCallback = Callback { cameras }.into();
        // SAFETY: monitor and callback are created and released on the COM worker.
        self.monitor = unsafe { MFCreateSensorActivityMonitor(&callback) }.ok();
        self.monitor
            .as_ref()
            .is_some_and(|monitor| unsafe { monitor.Start().is_ok() })
    }
    fn wait(&mut self) -> bool {
        // Microsoft's documented initial query window. A completed healthy window
        // permits omitted idle reports; cancellation and errors remain unknown.
        self.stopping = self.stopped.recv_timeout(Duration::from_millis(500))
            != Err(mpsc::RecvTimeoutError::Timeout);
        !self.stopping
    }
    fn stop(&mut self) -> bool {
        let Some(monitor) = self.monitor.take() else {
            return false;
        };
        // SAFETY: this worker owns the monitor; finish after callbacks are stopped.
        unsafe {
            let stopped = monitor.Stop().is_ok();
            let shutdown = monitor
                .cast::<IMFShutdown>()
                .is_ok_and(|s| s.Shutdown().is_ok());
            stopped && shutdown
        }
    }
}

pub fn start() -> Box<dyn CameraPort> {
    let state = Arc::new(Mutex::new((unavailable(), Instant::now())));
    let output = Arc::clone(&state);
    let (stop, stopped) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        // SAFETY: initialization, monitor lifetime, and teardown stay on this worker.
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                return;
            }
            if MFStartup(MF_VERSION, MFSTARTUP_LITE).is_err() {
                CoUninitialize();
                return;
            }
            let mut source = NativeQuery {
                monitor: None,
                stopped: &stopped,
                stopping: false,
            };
            while !source.stopping {
                let observation = query::sample(&mut source);
                if let Ok(mut state) = output.lock() {
                    *state = (observation, Instant::now());
                }
            }
            let _ = MFShutdown();
            CoUninitialize();
        }
    });
    Box::new(Monitor {
        state,
        stop,
        thread: Some(thread),
    })
}
