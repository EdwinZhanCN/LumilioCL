use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    /// No graphics adapter, hardware or software.
    NoGpu,
    Parse(String),
    Pack(String),
    Mesh(String),
    Render(String),
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGpu => f.write_str("这台电脑没有可用的显卡，没法显示 3D 预览"),
            Self::Parse(e) => write!(f, "读不懂这个投影文件：{e}"),
            Self::Pack(e) => write!(f, "游戏的贴图包读不出来：{e}"),
            Self::Mesh(e) => write!(f, "生成 3D 模型失败：{e}"),
            Self::Render(e) => write!(f, "绘制失败：{e}"),
        }
    }
}

impl std::error::Error for SceneError {}

impl SceneError {
    pub(crate) fn from_render(error: nucleation::rendering::RenderError) -> Self {
        use nucleation::rendering::RenderError;
        match error {
            RenderError::NoGpuAdapter => Self::NoGpu,
            other => Self::Render(other.to_string()),
        }
    }
}
