use crate::{
    app::{effect::Task, jobs::*, state::View},
    desktop::Eder,
    domain::{document::Entry, reading_position::ReadingPosition},
};
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use std::{path::PathBuf, sync::atomic::Ordering};
use theme::*;

mod annotations;
mod library;
mod navigation;
mod reader;
mod settings;
pub(crate) mod state;
pub(crate) mod textures;
pub(crate) mod theme;
