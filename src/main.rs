mod infrastructure;
mod models;
mod repository;
mod service;

use std::error::Error;
use std::rc::Rc;

use infrastructure::ui::{self, ResultRow};
use repository::AppRepository;
use service::AppSearch;
use slint::{ComponentHandle, ModelRc, VecModel};

#[cfg(target_os = "linux")]
fn platform_repository() -> impl AppRepository {
    infrastructure::linux::LinuxAppRepository::new()
}

#[cfg(target_os = "windows")]
fn platform_repository() -> impl AppRepository {
    infrastructure::windows::WindowsAppRepository::new()
}

fn main() -> Result<(), Box<dyn Error>> {
    let search = AppSearch::new(&platform_repository())?;
    let window = ui::create_window()?;

    let results = Rc::new(VecModel::<ResultRow>::default());
    window.set_results(ModelRc::from(results.clone()));
    window.on_query_changed(move |query| {
        let rows: Vec<ResultRow> = search
            .search(&query)
            .into_iter()
            .map(|app| ResultRow {
                name: app.name.into(),
            })
            .collect();
        results.set_vec(rows);
    });

    ui::center_on_startup(&window);
    window.run()?;
    Ok(())
}
