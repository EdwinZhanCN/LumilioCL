use std::path::{Path, PathBuf};

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::error::ServiceError;
use crate::java::InstanceJava;
use crate::java::downloads::{JavaBuild, JavaSource};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn install_java(
        &self,
        required: Option<u32>,
        build: JavaBuild,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        self.java.install_java(required, build, cancel).await
    }

    pub async fn install_instance_java(
        &self,
        id: &str,
        required: Option<u32>,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        self.java.install_instance_java(id, required, cancel).await
    }

    pub async fn install_java_from(
        &self,
        source: JavaSource,
        required: Option<u32>,
        build: JavaBuild,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        self.java
            .install_java_from(source, required, build, cancel)
            .await
    }

    pub async fn instance_java(&self, id: &str) -> Result<InstanceJava, ServiceError> {
        self.java.instance_java(id).await
    }

    pub async fn remove_java(&self, home: &Path) -> Result<(), ServiceError> {
        self.java.remove_java(home).await
    }

    pub async fn java_offers(&self) -> Vec<(JavaSource, Vec<u32>)> {
        self.java.java_offers().await
    }

    pub async fn java_installations(&self) -> Vec<(crate::java::JavaRuntime, bool)> {
        self.java.java_installations().await
    }

    pub async fn set_java_source(
        &self,
        source: crate::java::downloads::JavaSource,
    ) -> Result<(), ServiceError> {
        self.java.set_java_source(source).await
    }

    pub async fn set_auto_install_java(&self, enabled: bool) -> Result<(), ServiceError> {
        self.java.set_auto_install_java(enabled).await
    }

    pub async fn set_java_disabled(&self, home: &Path, disabled: bool) -> Result<(), ServiceError> {
        self.java.set_java_disabled(home, disabled).await
    }

    pub async fn add_java(&self, chosen: &Path) -> Result<crate::java::JavaRuntime, ServiceError> {
        self.java.add_java(chosen).await
    }

    pub async fn set_java_roots(&self, roots: Vec<PathBuf>) -> Result<(), ServiceError> {
        self.java.set_java_roots(roots).await
    }
}
