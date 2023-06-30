use std::path::PathBuf;

use cursive::event::{Event, Key};
use cursive::view::Resizable;
use cursive::Cursive;

use crate::args::Args;
use crate::commands::*;
use crate::player::{Player, Size};
use crate::player_view::PlayerView;
use crate::utils::*;

#[derive(Clone)]
pub struct App {
    pub path: PathBuf,
    pub path_string: String,
    pub search_dir: SearchDir,
    pub search_mode: SearchMode,
    pub fd_available: bool,
    pub fuzzy_cmd: String,
    is_first_run: bool,
}

impl App {
    fn try_new() -> Result<Self, anyhow::Error> {
        let (path, path_string) = Args::parse_path()?;
        let search_dir = SearchDir::get_from(&path);
        let search_mode = SearchMode::get_from(&path);

        let app = Self {
            path: path,
            path_string: path_string,
            search_dir: search_dir,
            search_mode: search_mode,
            fd_available: env_var_includes(&["fd"]),
            fuzzy_cmd: get_fuzzy_cmd(),
            is_first_run: true,
        };

        Ok(app)
    }

    pub fn run() -> Result<(), anyhow::Error> {
        let mut app = App::try_new()?;

        // Clone for use in pre-event callback.
        let app_clone = app.clone();

        let mut cursive = cursive::default();

        // Set style and background color.
        cursive
            .load_toml(include_str!("assets/style.toml"))
            .expect("style.toml should be located in assets directory");

        // Initialize the player and player view.
        app.init_player(&mut cursive)?;

        // Create a new player instance from a random selection.
        cursive.set_on_pre_event(Event::Char('r'), move |c: &mut Cursive| {
            app_clone.new_random_search(c);
        });

        // Create a new player instance from the previous selection.
        cursive.set_on_pre_event(Event::Char('R'), move |c: &mut Cursive| {
            previous_search(c);
        });

        // Create a new player instance from a fuzzy selection.
        cursive.set_on_pre_event(Event::Key(Key::Tab), move |c: &mut Cursive| {
            app.new_fuzzy_search(c)
        });

        // Quit the app.
        cursive.set_on_pre_event(Event::Char('q'), move |c: &mut Cursive| c.quit());

        // Set fps to lowest value that looks steady.
        cursive.set_fps(16);
        cursive.run();

        clear_terminal()?;

        Ok(())
    }

    fn init_player(&mut self, c: &mut Cursive) -> Result<(), anyhow::Error> {
        // Add dummy data to user data (required when calling `load_player`).
        c.set_user_data(vec![PathBuf::new()]);

        if self.search_mode == SearchMode::Fuzzy {
            self.new_fuzzy_search(c)
        } else {
            let (player, size) = Player::new(self.path.clone())?;
            load_player((player, size), c);
        }

        // Replace the dummy user data with a copy of the initial path.
        // This ensures we have only valid paths in user data (required
        // when calling `previous_selection` before making a selection).
        c.with_user_data(|paths: &mut Vec<PathBuf>| {
            let p = paths.last().expect("path set on init");
            paths.push(p.clone());
            paths.remove(0);
        });

        self.is_first_run = false;
        Ok(())
    }

    fn new_fuzzy_search(&self, c: &mut Cursive) {
        if self.search_mode == SearchMode::NonFuzzy {
            return;
        }

        let fuzzy_path = get_fuzzy_path(&self);
        let curr_path = c
            .user_data::<Vec<PathBuf>>()
            .expect("user data should be set on init")
            .last()
            .expect("current path is the last entry");

        let mut path = self.path.clone();
        // Push an empty path to append a trailing slash.
        path.push("");

        if fuzzy_path.eq(&path) || fuzzy_path.eq(curr_path) {
            if self.is_first_run {
                std::process::exit(1);
            } else {
                c.clear()
            }
        } else if let Ok((player, size)) = Player::new(fuzzy_path) {
            load_player((player, size), c)
        }
    }

    fn new_random_search(&self, c: &mut Cursive) {
        let dir_count = get_dir_count(&self);
        let mut count = 0;

        while count < 10 {
            let random_path = get_random_path(&self, dir_count);
            let curr_path = c
                .user_data::<Vec<PathBuf>>()
                .expect("user data should be set on init")
                .last()
                .expect("current path is the last entry");

            if random_path.eq(curr_path) {
                count += 1
            } else if let Ok((player, size)) = Player::new(random_path) {
                load_player((player, size), c);
                break;
            } else {
                count += 1;
            }
        }
    }
}

fn load_player((player, size): (Player, Size), c: &mut Cursive) {
    c.with_user_data(|paths: &mut Vec<PathBuf>| {
        paths.push(player.path.clone());
        if paths.len() > 2 {
            paths.remove(0);
        }
    });
    c.pop_layer();
    c.add_layer(
        PlayerView::new(player)
            .full_width()
            .max_width(std::cmp::max(size.0, 53))
            .fixed_height(size.1),
    );
}

fn previous_search(c: &mut Cursive) {
    let prev_path = c
        .user_data::<Vec<PathBuf>>()
        .expect("user data should be set on init")
        .first()
        .expect("previous path is at index 0 in user data");

    let (player, size) =
        Player::new(prev_path.clone()).expect("player created from this path previously");

    load_player((player, size), c);
}

#[derive(Clone, Copy, PartialEq)]
pub enum SearchMode {
    Fuzzy,
    NonFuzzy,
}

impl SearchMode {
    pub fn get_from(path: &PathBuf) -> Self {
        let fuzzy_available = env_var_includes(&["fzf"]) || env_var_includes(&["sk"]);
        match has_child_dir(path) && fuzzy_available {
            true => SearchMode::Fuzzy,
            false => SearchMode::NonFuzzy,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SearchDir {
    CurrentDir,
    PathArg,
}

impl SearchDir {
    pub fn get_from(path: &PathBuf) -> Self {
        match *path
            == std::env::current_dir().expect("current directory should exist and be accessible")
        {
            true => SearchDir::CurrentDir,
            false => SearchDir::PathArg,
        }
    }
}
