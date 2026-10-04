use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const OFFICIAL_PUBLISHER: &str = "MiguelTVMS/speaker-volume-bridge";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionEdition {
    DirectMacos,
    DirectWindows,
    MicrosoftStore,
    MacAppStore,
    Debian,
    WindowsSideload,
    Development,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Stable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationArchitecture {
    Aarch64,
    X86_64,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledDistribution {
    pub edition: DistributionEdition,
    pub version: String,
    pub architecture: ApplicationArchitecture,
    pub channel: ReleaseChannel,
    pub check_supported: bool,
    pub direct_install_supported: bool,
}

impl InstalledDistribution {
    fn new(
        edition: DistributionEdition,
        version: &str,
        architecture: ApplicationArchitecture,
    ) -> Self {
        Self {
            edition,
            version: version.into(),
            architecture,
            channel: ReleaseChannel::Stable,
            check_supported: matches!(
                edition,
                DistributionEdition::DirectMacos
                    | DistributionEdition::DirectWindows
                    | DistributionEdition::MicrosoftStore
                    | DistributionEdition::MacAppStore
                    | DistributionEdition::Debian
            ),
            direct_install_supported: false,
        }
    }
}

pub trait DistributionResolver: Send + Sync {
    fn resolve(&self) -> Result<InstalledDistribution, ResolveError>;
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("installed distribution evidence is unavailable")]
pub struct ResolveError;

pub fn resolve_at_startup(resolver: &dyn DistributionResolver) -> InstalledDistribution {
    resolver.resolve().unwrap_or_else(|error| {
        tracing::warn!(%error, "update distribution resolution failed");
        InstalledDistribution::new(
            DistributionEdition::Unknown,
            env!("CARGO_PKG_VERSION"),
            application_architecture(),
        )
    })
}

#[allow(clippy::struct_excessive_bools)] // Independent OS evidence is injected explicitly in resolver tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformEvidence {
    pub packaged: bool,
    pub store_signed: bool,
    pub app_store_receipt: bool,
    pub official_debian_package: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageProvenance {
    schema_version: u32,
    edition: ProvenanceEdition,
    publisher: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProvenanceEdition {
    DirectMacos,
    DirectWindows,
    MicrosoftStore,
    MacAppStore,
    Debian,
}

pub struct InstalledDistributionResolver {
    provenance: Option<PackageProvenance>,
    evidence: PlatformEvidence,
    development: bool,
    version: &'static str,
    architecture: ApplicationArchitecture,
}

impl InstalledDistributionResolver {
    pub fn from_runtime(resource_dir: &Path, development: bool) -> Self {
        let provenance = fs::read(resource_dir.join("distribution.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        Self {
            provenance,
            evidence: platform_evidence(resource_dir),
            development,
            version: env!("CARGO_PKG_VERSION"),
            architecture: application_architecture(),
        }
    }

    #[cfg(test)]
    fn from_evidence(
        provenance: Option<ProvenanceEdition>,
        evidence: PlatformEvidence,
        development: bool,
        architecture: ApplicationArchitecture,
    ) -> Self {
        Self {
            provenance: provenance.map(|edition| PackageProvenance {
                schema_version: 1,
                edition,
                publisher: OFFICIAL_PUBLISHER.into(),
            }),
            evidence,
            development,
            version: env!("CARGO_PKG_VERSION"),
            architecture,
        }
    }

    fn resolved_edition(&self) -> DistributionEdition {
        if self.development {
            return DistributionEdition::Development;
        }
        let Some(provenance) = &self.provenance else {
            return if self.evidence.packaged {
                DistributionEdition::WindowsSideload
            } else {
                DistributionEdition::Unknown
            };
        };
        if provenance.schema_version != 1 || provenance.publisher != OFFICIAL_PUBLISHER {
            return DistributionEdition::Unknown;
        }
        match provenance.edition {
            ProvenanceEdition::DirectMacos if !self.evidence.app_store_receipt => {
                DistributionEdition::DirectMacos
            }
            ProvenanceEdition::MacAppStore if self.evidence.app_store_receipt => {
                DistributionEdition::MacAppStore
            }
            ProvenanceEdition::DirectWindows if !self.evidence.packaged => {
                DistributionEdition::DirectWindows
            }
            ProvenanceEdition::MicrosoftStore
                if self.evidence.packaged && self.evidence.store_signed =>
            {
                DistributionEdition::MicrosoftStore
            }
            ProvenanceEdition::MicrosoftStore | ProvenanceEdition::DirectWindows
                if self.evidence.packaged =>
            {
                DistributionEdition::WindowsSideload
            }
            ProvenanceEdition::Debian if self.evidence.official_debian_package => {
                DistributionEdition::Debian
            }
            _ => DistributionEdition::Unknown,
        }
    }
}

impl DistributionResolver for InstalledDistributionResolver {
    fn resolve(&self) -> Result<InstalledDistribution, ResolveError> {
        Ok(InstalledDistribution::new(
            self.resolved_edition(),
            self.version,
            self.architecture,
        ))
    }
}

const fn application_architecture() -> ApplicationArchitecture {
    if cfg!(target_arch = "aarch64") {
        ApplicationArchitecture::Aarch64
    } else if cfg!(target_arch = "x86_64") {
        ApplicationArchitecture::X86_64
    } else {
        ApplicationArchitecture::Unknown
    }
}

#[cfg(target_os = "macos")]
fn platform_evidence(resource_dir: &Path) -> PlatformEvidence {
    PlatformEvidence {
        app_store_receipt: resource_dir.join("../_MASReceipt/receipt").is_file(),
        ..PlatformEvidence::default()
    }
}

#[cfg(windows)]
fn platform_evidence(_resource_dir: &Path) -> PlatformEvidence {
    use windows::ApplicationModel::{Package, PackageSignatureKind};
    let Ok(package) = Package::Current() else {
        return PlatformEvidence::default();
    };
    PlatformEvidence {
        packaged: true,
        store_signed: package
            .SignatureKind()
            .is_ok_and(|kind| kind == PackageSignatureKind::Store),
        ..PlatformEvidence::default()
    }
}

#[cfg(target_os = "linux")]
fn platform_evidence(_resource_dir: &Path) -> PlatformEvidence {
    let official = std::process::Command::new("dpkg-query")
        .args([
            "-W",
            "-f=${Package}|${Maintainer}|${Homepage}",
            "speaker-volume-bridge",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|value| {
            value.starts_with("speaker-volume-bridge|")
                && value.contains("|https://github.com/migueltvms/speaker-volume-bridge")
        });
    PlatformEvidence {
        official_debian_package: official,
        ..PlatformEvidence::default()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn platform_evidence(_resource_dir: &Path) -> PlatformEvidence {
    PlatformEvidence::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        provenance: Option<ProvenanceEdition>,
        evidence: PlatformEvidence,
        development: bool,
        architecture: ApplicationArchitecture,
    ) -> InstalledDistribution {
        InstalledDistributionResolver::from_evidence(
            provenance,
            evidence,
            development,
            architecture,
        )
        .resolve()
        .unwrap()
    }

    #[test]
    fn recognized_editions_require_matching_package_and_platform_evidence() {
        let cases = [
            (
                ProvenanceEdition::DirectMacos,
                PlatformEvidence::default(),
                DistributionEdition::DirectMacos,
            ),
            (
                ProvenanceEdition::MacAppStore,
                PlatformEvidence {
                    app_store_receipt: true,
                    ..PlatformEvidence::default()
                },
                DistributionEdition::MacAppStore,
            ),
            (
                ProvenanceEdition::DirectWindows,
                PlatformEvidence::default(),
                DistributionEdition::DirectWindows,
            ),
            (
                ProvenanceEdition::MicrosoftStore,
                PlatformEvidence {
                    packaged: true,
                    store_signed: true,
                    ..PlatformEvidence::default()
                },
                DistributionEdition::MicrosoftStore,
            ),
            (
                ProvenanceEdition::Debian,
                PlatformEvidence {
                    official_debian_package: true,
                    ..PlatformEvidence::default()
                },
                DistributionEdition::Debian,
            ),
        ];
        for (provenance, evidence, edition) in cases {
            let result = resolve(
                Some(provenance),
                evidence,
                false,
                ApplicationArchitecture::Aarch64,
            );
            assert_eq!(result.edition, edition);
            assert!(result.check_supported);
            assert!(!result.direct_install_supported);
        }
    }

    #[test]
    fn reused_binary_resolves_from_package_metadata_not_compile_time() {
        let direct = resolve(
            Some(ProvenanceEdition::DirectWindows),
            PlatformEvidence::default(),
            false,
            ApplicationArchitecture::X86_64,
        );
        let store = resolve(
            Some(ProvenanceEdition::MicrosoftStore),
            PlatformEvidence {
                packaged: true,
                store_signed: true,
                ..PlatformEvidence::default()
            },
            false,
            ApplicationArchitecture::X86_64,
        );
        assert_eq!(direct.edition, DistributionEdition::DirectWindows);
        assert_eq!(store.edition, DistributionEdition::MicrosoftStore);
        assert_eq!(direct.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn weak_platform_evidence_never_proves_an_official_edition() {
        assert_eq!(
            resolve(
                Some(ProvenanceEdition::MicrosoftStore),
                PlatformEvidence {
                    packaged: true,
                    ..PlatformEvidence::default()
                },
                false,
                ApplicationArchitecture::X86_64
            )
            .edition,
            DistributionEdition::WindowsSideload
        );
        assert_eq!(
            resolve(
                Some(ProvenanceEdition::MacAppStore),
                PlatformEvidence::default(),
                false,
                ApplicationArchitecture::Aarch64
            )
            .edition,
            DistributionEdition::Unknown
        );
        assert_eq!(
            resolve(
                Some(ProvenanceEdition::Debian),
                PlatformEvidence::default(),
                false,
                ApplicationArchitecture::X86_64
            )
            .edition,
            DistributionEdition::Unknown
        );
    }

    #[test]
    fn absent_conflicting_and_development_evidence_are_conservative() {
        assert_eq!(
            resolve(
                None,
                PlatformEvidence::default(),
                false,
                ApplicationArchitecture::X86_64
            )
            .edition,
            DistributionEdition::Unknown
        );
        assert_eq!(
            resolve(
                Some(ProvenanceEdition::DirectWindows),
                PlatformEvidence {
                    packaged: true,
                    store_signed: true,
                    ..PlatformEvidence::default()
                },
                false,
                ApplicationArchitecture::X86_64
            )
            .edition,
            DistributionEdition::WindowsSideload
        );
        let demo = resolve(
            Some(ProvenanceEdition::DirectMacos),
            PlatformEvidence::default(),
            true,
            ApplicationArchitecture::Aarch64,
        );
        assert_eq!(demo.edition, DistributionEdition::Development);
        assert!(!demo.check_supported);
    }

    #[test]
    fn application_architecture_is_independent_from_host_architecture() {
        let result = resolve(
            Some(ProvenanceEdition::DirectWindows),
            PlatformEvidence::default(),
            false,
            ApplicationArchitecture::X86_64,
        );
        assert_eq!(result.architecture, ApplicationArchitecture::X86_64);
    }

    #[test]
    fn resolver_errors_fall_back_without_preventing_startup() {
        struct FailingResolver;
        impl DistributionResolver for FailingResolver {
            fn resolve(&self) -> Result<InstalledDistribution, ResolveError> {
                Err(ResolveError)
            }
        }
        let result = resolve_at_startup(&FailingResolver);
        assert_eq!(result.edition, DistributionEdition::Unknown);
        assert!(!result.check_supported);
    }
}
