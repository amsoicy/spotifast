//! The artist page.

use std::sync::Arc;

use crate::api::models::{Artist, PlayableItem, pick_image};
use crate::app::App;
use crate::i18n::{gettext, ngettext, pgettext};
use crate::model::{Action, DiscographyFilter, Loadable, Page, RowContext};
use crate::theme::{self, Icon};
use crate::util;

use super::collection::{Hero, hero, hero_images};
use super::widgets::{self, TrackRow};

pub fn show(app: &mut App, ui: &mut egui::Ui, id: &str) {
    if !app.artist_pages.contains_key(id) {
        app.ensure_loaded(Page::Artist(id.to_string()));
    }
    if !app.library.liked.loaded_once && !app.library.liked.loading {
        app.ensure_loaded(Page::LikedSongs);
    }
    let Some(page) = app.artist_pages.remove(id) else {
        return;
    };
    let preview =
        super::loading_preview(ui.ctx(), id, &page.artist, || app.known_artist(id).cloned());
    let palette = app.palette;
    let locale = app.locale;
    // Liked songs credited to this artist, matched on id so similarly
    // named artists, albums, or songs never slip in.
    let liked: Vec<PlayableItem> = match &page.artist {
        Loadable::Loaded(artist) => liked_songs(app, artist),
        Loadable::Loading | Loadable::NotLoaded | Loadable::Failed(_) => Vec::new(),
    };
    match &page.artist {
        Loadable::Loaded(artist) => {
            artist_hero(app, ui, artist, preview.as_deref());
            artist_actions(app, ui, artist);

            // Most popular.
            theme::section_title(ui, &palette, &gettext(locale, "Most popular"));
            ui.add_space(4.0);
            match &page.top_tracks {
                Loadable::Loaded(tracks) if !tracks.is_empty() => {
                    let uris: Arc<[String]> = tracks
                        .iter()
                        .map(|track| track.uri.clone())
                        .collect::<Vec<_>>()
                        .into();
                    let context = RowContext::Uris(Arc::clone(&uris));
                    let items: Vec<PlayableItem> =
                        tracks.iter().cloned().map(PlayableItem::Track).collect();
                    let limit = if page.show_all_top { items.len() } else { 5 };
                    for (index, item) in items.iter().take(limit).enumerate() {
                        widgets::track_row(
                            ui,
                            app,
                            TrackRow {
                                index,
                                number: Some(index + 1),
                                item,
                                context: &context,
                                show_cover: !app.settings.tracklist_compact,
                                show_album: false,
                                added_at: None,
                                added_by: None,
                                show_added_by: false,
                                compact: false,
                                thin: app.settings.tracklist_compact,
                                shift: 0.0,
                                picked: false,
                                picked_songs: &[],
                            },
                        );
                    }
                    if items.len() > 5 {
                        ui.add_space(6.0);
                        if theme::soft_button(
                            ui,
                            &palette,
                            None,
                            &if page.show_all_top {
                                gettext(locale, "Show less")
                            } else {
                                gettext(locale, "See more")
                            },
                            false,
                        )
                        .clicked()
                        {
                            app.actions.push(Action::ToggleShowAllTop(id.to_string()));
                        }
                    }
                }
                Loadable::Loaded(_) => {
                    theme::subtle(
                        ui,
                        &palette,
                        &gettext(locale, "No most popular songs to show."),
                    );
                }
                Loadable::Loading | Loadable::NotLoaded => {
                    widgets::loading_row(ui, &palette, app.locale)
                }
                Loadable::Failed(error) => {
                    let error = error.clone();
                    widgets::error_row(ui, app, &error, None);
                }
            }
            ui.add_space(20.0);

            // Liked songs by this artist, from the loaded Liked Songs.
            if !liked.is_empty() {
                theme::section_title(ui, &palette, &gettext(locale, "Liked songs"));
                ui.add_space(4.0);
                let uris: Arc<[String]> = liked
                    .iter()
                    .map(|item| item.uri().to_string())
                    .collect::<Vec<_>>()
                    .into();
                let context = RowContext::Uris(Arc::clone(&uris));
                for (index, item) in liked.iter().take(5).enumerate() {
                    widgets::track_row(
                        ui,
                        app,
                        TrackRow {
                            index,
                            number: Some(index + 1),
                            item,
                            context: &context,
                            show_cover: !app.settings.tracklist_compact,
                            show_album: false,
                            added_at: None,
                            added_by: None,
                            show_added_by: false,
                            compact: false,
                            thin: app.settings.tracklist_compact,
                            shift: 0.0,
                            picked: false,
                            picked_songs: &[],
                        },
                    );
                }
                if liked.len() > 5 {
                    ui.add_space(6.0);
                    if theme::soft_button(ui, &palette, None, &gettext(locale, "See more"), false)
                        .clicked()
                    {
                        app.actions.push(Action::OpenArtistLiked(id.to_string()));
                    }
                }
                ui.add_space(20.0);
            }

            // Discography.
            theme::section_title(ui, &palette, &gettext(locale, "Discography"));
            ui.add_space(6.0);
            let labels: Vec<_> = DiscographyFilter::ALL
                .iter()
                .map(|f| (*f, f.label(locale)))
                .collect();
            let options: Vec<(DiscographyFilter, &str)> = labels
                .iter()
                .map(|(filter, label)| (*filter, label.as_ref()))
                .collect();
            if let Some(filter) = widgets::chips(ui, &palette, &options, page.filter) {
                app.actions.push(Action::SetDiscographyFilter {
                    artist_id: id.to_string(),
                    filter,
                });
            }
            ui.add_space(10.0);
            match page.albums.get(page.filter.groups()) {
                Some(list) => {
                    let mut seen = std::collections::HashSet::new();
                    let albums: Vec<_> = list
                        .items
                        .iter()
                        .filter(|album| seen.insert(album.name.to_lowercase()))
                        .collect();
                    widgets::grid(ui, |ui| {
                        for album in &albums {
                            let subtitle = format!(
                                "{} • {}",
                                album.year().unwrap_or(""),
                                app.album_kind_label(album)
                            );
                            let playing_here = app.playing_context_uri().as_deref()
                                == Some(album.uri.as_str())
                                && app.believed_playing();
                            let card = widgets::card(
                                ui,
                                app,
                                pick_image(&album.images, 640),
                                &album.name,
                                subtitle.trim_start_matches(" • "),
                                widgets::CardCover::square(playing_here),
                            );
                            if card.play {
                                if playing_here {
                                    app.actions.push(Action::TogglePlay);
                                } else {
                                    app.actions.push(Action::PlayContext {
                                        uri: album.uri.clone(),
                                        offset_uri: None,
                                        offset_index: None,
                                    });
                                }
                            }
                            if card.clicked {
                                app.actions
                                    .push(Action::Open(Page::Album(album.id.clone())));
                            }
                            egui::Popup::context_menu(&card.response)
                                .id(ui.make_persistent_id(("discography-menu", &album.uri)))
                                .frame(widgets::menu_frame(&palette))
                                .show(|ui| {
                                    widgets::context_menu_items(
                                        ui,
                                        app,
                                        &album.uri,
                                        &album.name,
                                        None,
                                    )
                                });
                        }
                    });
                    if list.loading {
                        widgets::loading_row(ui, &palette, app.locale);
                    } else if let Some(error) = &list.error {
                        let error = error.clone();
                        widgets::error_row(ui, app, &error, None);
                    } else if list.items.is_empty() {
                        theme::subtle(ui, &palette, &gettext(locale, "Nothing in this category."));
                    } else if list.can_load_more() {
                        ui.add_space(8.0);
                        if theme::soft_button(
                            ui,
                            &palette,
                            None,
                            &gettext(locale, "Load more"),
                            false,
                        )
                        .clicked()
                        {
                            app.actions
                                .push(Action::LoadMoreArtistAlbums(id.to_string()));
                        }
                    }
                }
                None => widgets::loading_row(ui, &palette, app.locale),
            }
            ui.add_space(20.0);

            // Related.
            if let Loadable::Loaded(related) = &page.related
                && !related.is_empty()
            {
                let artist_label = gettext(locale, "Artist");
                let title = gettext(locale, "Fans also like");
                widgets::shelf(ui, &palette, "related", &title, |ui| {
                    for artist in related {
                        let playing_here = app.playing_context_uri().as_deref()
                            == Some(artist.uri.as_str())
                            && app.believed_playing();
                        let card = widgets::card(
                            ui,
                            app,
                            pick_image(&artist.images, 640),
                            &artist.name,
                            &artist_label,
                            widgets::CardCover::portrait(playing_here),
                        );
                        if card.play {
                            if playing_here {
                                app.actions.push(Action::TogglePlay);
                            } else {
                                app.actions.push(Action::PlayContext {
                                    uri: artist.uri.clone(),
                                    offset_uri: None,
                                    offset_index: None,
                                });
                            }
                        }
                        if card.clicked {
                            app.actions
                                .push(Action::Open(Page::Artist(artist.id.clone())));
                        }
                        egui::Popup::context_menu(&card.response)
                            .id(ui.make_persistent_id(("related-artist-menu", &artist.uri)))
                            .frame(widgets::menu_frame(&palette))
                            .show(|ui| {
                                widgets::context_menu_items(
                                    ui,
                                    app,
                                    &artist.uri,
                                    &artist.name,
                                    None,
                                )
                            });
                    }
                });
            }
        }
        Loadable::Loading | Loadable::NotLoaded => {
            if let Some(artist) = &preview {
                artist_hero(app, ui, artist, None);
                ui.add_enabled_ui(false, |ui| artist_actions(app, ui, artist));
            } else {
                ui.add_space(40.0);
            }
            widgets::loading_row(ui, &palette, app.locale);
        }
        Loadable::Failed(error) => {
            let error = error.clone();
            if let Some(artist) = &preview {
                artist_hero(app, ui, artist, None);
                ui.add_enabled_ui(false, |ui| artist_actions(app, ui, artist));
            } else {
                ui.add_space(40.0);
            }
            widgets::error_row(ui, app, &error, Some(Page::Artist(id.to_string())));
        }
    }
    if app.artist_liked_open.as_deref() == Some(id)
        && !liked.is_empty()
        && let Loadable::Loaded(artist) = &page.artist
    {
        liked_modal(app, ui, artist, &liked);
    }
    app.artist_pages.insert(id.to_string(), page);
}

/// Liked songs credited to this artist, matched on id so similarly
/// named artists, albums, or songs never slip in.
fn liked_songs(app: &App, artist: &Artist) -> Vec<PlayableItem> {
    app.library
        .liked
        .items
        .iter()
        .filter(|saved| {
            saved.track.artists.iter().any(|credit| {
                credit.id.as_deref() == Some(artist.id.as_str())
                    || credit.uri.as_deref() == Some(artist.uri.as_str())
            })
        })
        .map(|saved| PlayableItem::Track(saved.track.clone()))
        .collect()
}

/// The artist's liked songs on a card over the page. A click outside
/// the card closes it; double-clicking a song plays it where it sits.
fn liked_modal(app: &mut App, ui: &mut egui::Ui, artist: &Artist, liked: &[PlayableItem]) {
    let palette = app.palette;
    let locale = app.locale;
    let frame = egui::Frame::new()
        .fill(palette.overlay)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS + 4))
        .inner_margin(egui::Margin::same(24))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 40,
            spread: 0,
            color: palette.shadow,
        });
    let modal = egui::Modal::new(egui::Id::new(("artist-liked", &artist.id)))
        .frame(frame)
        .backdrop_color(egui::Color32::from_black_alpha(if palette.dark {
            150
        } else {
            80
        }))
        .show(ui.ctx(), |ui| {
            let width = (ui.ctx().content_rect().width() - 48.0).clamp(320.0, 520.0);
            ui.set_width(width);
            ui.horizontal(|ui| {
                theme::section_title(
                    ui,
                    &palette,
                    &gettext(locale, "Liked songs from {name}").replace("{name}", &artist.name),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::icon_button(
                        ui,
                        Icon::X,
                        26.0,
                        palette.secondary,
                        palette.text,
                        &gettext(locale, "Close"),
                    )
                    .clicked()
                    {
                        app.actions.push(Action::CloseArtistLiked);
                    }
                });
            });
            ui.add_space(8.0);
            let uris: Arc<[String]> = liked
                .iter()
                .map(|item| item.uri().to_string())
                .collect::<Vec<_>>()
                .into();
            let context = RowContext::Uris(uris);
            // Fixed list height, not a negotiated one: inside the
            // auto-sized card a max-height viewport converges small,
            // leaving rows painted where clicks cannot land.
            let list_height = (liked.len() as f32 * theme::ROW_HEIGHT).min(480.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), list_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (index, item) in liked.iter().enumerate() {
                            if liked_modal_row(ui, app, item, index, &context) {
                                app.actions.push(Action::PlayFromRow {
                                    context: context.clone(),
                                    uri: item.uri().to_string(),
                                    index: index as u32,
                                });
                            }
                        }
                    });
                },
            );
        });
    if modal.should_close() {
        app.actions.push(Action::CloseArtistLiked);
    }
}

/// One song in the liked-songs card: number, cover, title, and length.
/// Plain responses keep their own identities here, where the shared
/// track rows would collide with the section behind. Right-clicking
/// offers the ordinary song menu; double-clicking (or Enter on a
/// focused row) plays it; a single click only focuses.
/// Returns whether it was chosen to play.
fn liked_modal_row(
    ui: &mut egui::Ui,
    app: &mut App,
    item: &PlayableItem,
    index: usize,
    context: &RowContext,
) -> bool {
    let palette = app.palette;
    let locale = app.locale;
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, theme::ROW_HEIGHT), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            ui.is_enabled(),
            false,
            gettext(
                locale,
                // Translators: {title} is a song or episode name, {subtitle} its artists or podcast.
                "Play {title}, {subtitle}",
            )
            .replace("{title}", item.name())
            .replace("{subtitle}", &item.subtitle()),
        )
    });
    theme::focus_ring(ui, &response);
    if ui.is_rect_visible(rect) {
        if response.hovered() || response.has_focus() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(6), palette.surface_hover);
        }
        let middle = rect.center().y;
        let number = crate::bidi::layout_line(
            ui.painter(),
            format!("{}", index + 1),
            theme::regular(13.5),
            palette.dim,
        );
        ui.painter().galley(
            egui::pos2(rect.left() + 16.0, middle - number.size().y / 2.0),
            number,
            palette.dim,
        );
        super::widgets::paint_cover(
            ui,
            &palette,
            item.image(64),
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 72.0, middle),
                egui::Vec2::splat(40.0),
            ),
            4.0,
            Icon::Music,
            Some(app.backend.art()),
        );
        let duration = crate::bidi::layout_line(
            ui.painter(),
            util::format_duration_ms(item.duration_ms()),
            theme::regular(12.5),
            palette.secondary,
        );
        let duration_size = duration.size();
        // The bar keeps clear of the scrollbar at the row's edge.
        let right = rect.right() - 24.0;
        ui.painter().galley(
            egui::pos2(right - duration_size.x, middle - duration_size.y / 2.0),
            duration,
            palette.secondary,
        );
        let title = crate::bidi::layout(
            ui.painter(),
            item.name(),
            theme::medium(14.0),
            palette.text,
            (right - duration_size.x - 8.0 - (rect.left() + 100.0)).max(0.0),
            1,
            Some(crate::bidi::ELLIPSIS),
        );
        let title_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 100.0, middle - title.size().y / 2.0),
            egui::pos2(right - duration_size.x - 8.0, middle + title.size().y / 2.0),
        );
        ui.painter().galley(
            crate::bidi::galley_pos(title_rect, &title),
            title,
            palette.text,
        );
    }
    if response.double_clicked() {
        return true;
    }
    if response.has_focus()
        && ui.input(|input| {
            input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space)
        })
    {
        return true;
    }
    if response.clicked() {
        response.request_focus();
    }
    egui::Popup::context_menu(&response)
        .frame(widgets::menu_frame(&palette))
        .show(|ui| {
            widgets::item_menu(ui, app, item, Some(context), Some(index));
        });
    false
}

fn artist_hero(app: &mut App, ui: &mut egui::Ui, artist: &Artist, preview: Option<&Artist>) {
    let locale = app.locale;
    let mut byline = Vec::new();
    if let Some(followers) = &artist.followers {
        byline.push((
            ngettext(
                locale,
                // Translators: {count} is the number of people who follow an artist.
                "{count} follower",
                "{count} followers",
                u32::try_from(followers.total).unwrap_or(u32::MAX),
            )
            .replace("{count}", &util::format_count(followers.total)),
            None,
        ));
    }

    // Genres draw as pills on their own row under the followers.
    let pills: Vec<String> = artist.genres.iter().take(3).cloned().collect();
    let images = hero_images(
        &artist.images,
        preview.map(|artist| artist.images.as_slice()),
        false,
    );
    hero(
        app,
        ui,
        Hero {
            images,
            liked: false,
            kind: gettext(locale, "Artist"),
            title: &artist.name,
            description: None,
            byline,
            pills,
            round: true,
        },
    );
}

fn artist_actions(app: &mut App, ui: &mut egui::Ui, artist: &Artist) {
    let palette = app.palette;
    let locale = app.locale;
    let following = app.is_saved(&artist.uri).unwrap_or(false);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        if app.play_pending(&artist.uri) {
            theme::circle_spinner(
                ui,
                56.0,
                palette.accent,
                palette.on_accent,
                &gettext(locale, "Starting…"),
            );
        } else if theme::circle_button(
            ui,
            Icon::PlayFilled,
            56.0,
            palette.accent,
            palette.accent_hover,
            palette.on_accent,
            &gettext(locale, "Play"),
        )
        .clicked()
        {
            app.actions.push(Action::PlayContext {
                uri: artist.uri.clone(),
                offset_uri: None,
                offset_index: None,
            });
        }
        if theme::pill_button(
            ui,
            &palette,
            &if following {
                pgettext(locale, "artist", "Following")
            } else {
                pgettext(locale, "artist", "Follow")
            },
            false,
        )
        .clicked()
        {
            app.actions.push(Action::ToggleSaved(artist.uri.clone()));
        }
        let more = theme::icon_button(
            ui,
            Icon::Ellipsis,
            26.0,
            palette.secondary,
            palette.text,
            &gettext(locale, "More"),
        );
        egui::Popup::menu(&more)
            .frame(widgets::menu_frame(&palette))
            .show(|ui| widgets::context_menu_items(ui, app, &artist.uri, &artist.name, None));
    });
    ui.add_space(20.0);
}
