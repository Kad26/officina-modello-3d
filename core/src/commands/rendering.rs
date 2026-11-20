//! Pictures of the model and where it stands: renders, thumbnails, the sun and the environment.
//! Arms of `Engine::run`; each works on the model being edited (inside `edit`, the open component's definition).

use super::*;

impl Engine {
    pub(super) fn run_rendering(&mut self, cmd: Command) -> Result<Value> {
        let m: &mut Model = active_model!(self);
        Ok(match cmd {
            Command::ImportVrayLights { path } => serde_json::to_value(crate::vrscene::read(std::path::Path::new(&path))?)?,
            Command::Photoreal { out, samples, bounces, mut sun_azimuth, mut sun_altitude, sun_strength, shadow_angle, mut area_lights, use_embedded_lights, denoise, exposure, sky_strength, background, ground, ground_color, seamless_ground, warmth, date, time, utc_offset, progressive, restart, opts } => {
                let model = &self.model;
                let embedded = if use_embedded_lights { crate::skp_lights::world(model) } else { Default::default() };
                let capacity = crate::skp_lights::MAX_LIGHTS.saturating_sub(area_lights.len());
                let embedded_count = embedded.area_lights.len().min(capacity);
                area_lights.extend(embedded.area_lights.into_iter().take(capacity).map(|light| crate::pt::AreaLightSpec {
                    position: light.position, target: light.target, u_axis: Some(light.u_axis), width: light.width,
                    height: light.height, intensity: light.intensity, color: light.color, uniform_emission: light.uniform_emission,
                    directionality: light.directionality, shadows: light.shadows,
                }));
                let embedded_ambient = sky_strength > 0.0 && embedded.sky_color.is_some_and(|color| color.iter().any(|&v| v > 0.0));
                let mut sun_strength = sun_strength.unwrap_or_else(|| {
                    if (embedded_count > 0 || embedded_ambient) && embedded.sun_enabled == Some(false) { 0.0 } else { command::pt_sun() }
                });
                let area_lights = crate::pt::area_lights(area_lights)?;
                let mut sun_note = None;
                if date.is_some() || time.is_some() {
                    let s = sun_at(model, date.as_deref(), time.as_deref(), utc_offset)?;
                    if s.altitude < 1.0 {
                        if area_lights.is_empty() && !embedded_ambient {
                            bail!("the sun is down then ({:.0}° below the horizon); pick a daytime hour", -s.altitude);
                        }
                        sun_strength = 0.0;
                    }
                    sun_azimuth = Some(s.azimuth);
                    sun_altitude = s.altitude;
                    sun_note = Some(serde_json::to_value(&s)?);
                }
                let camera = crate::pt::camera(model, &opts);
                let saved = opts.camera.is_none().then(|| model.views.iter().find(|view| view.name == opts.view)).flatten();
                let camera_up = opts.camera_up.or_else(|| saved.and_then(|view| view.up));
                let parallel = opts.parallel || saved.is_some_and(|view| view.parallel == Some(true));
                let ortho_height = parallel.then(|| opts.ortho_height.or_else(|| saved.and_then(|view| view.ortho_height)).unwrap_or_else(|| {
                    let distance = glam::DVec3::from(camera.eye).distance(glam::DVec3::from(camera.target));
                    2.0 * distance * (camera.fov.to_radians() * 0.5).tan()
                }));
                // flat figures turn to face this camera
                let turned = model.facing(glam::DVec3::from(camera.eye));
                let model = turned.as_ref().unwrap_or(model);
                let po = crate::pt::PtOpts {
                    width: opts.width,
                    height: opts.height,
                    samples: samples.clamp(1, 4096),
                    bounces: bounces.clamp(1, 16),
                    camera: camera.clone(),
                    camera_up,
                    ortho_height,
                    sun: crate::pt::sun_dir(
                        sun_azimuth.unwrap_or_else(|| {
                            // three-quarter light: over the camera's right shoulder, so the face you see is lit and shadows fall beside it
                            let (e, t) = (glam::DVec3::from(camera.eye), glam::DVec3::from(camera.target));
                            let back = e - t;
                            let az = back.x.atan2(back.y).to_degrees();
                            az + 45.0
                        }),
                        sun_altitude,
                    ),
                    sun_strength,
                    shadow_angle: shadow_angle.map(|angle| angle.clamp(0.0, 20.0)),
                    area_lights,
                    denoise,
                    exposure: exposure.clamp(0.01, 64.0),
                    sky_strength: sky_strength.clamp(0.0, 10.0),
                    sky_color: embedded.sky_color,
                    white_balance: embedded.white_balance,
                    reinhard: embedded.reinhard,
                    background: background.as_deref().map(hex_rgb).transpose()?,
                    ground,
                    ground_color: ground_color.as_deref().map(hex_rgb).transpose()?,
                    seamless_ground,
                    warmth: warmth.clamp(-1.0, 1.0),
                }
                .limited();
                let t0 = std::time::Instant::now();
                let mut spp = po.samples;
                let img = if progressive {
                    // same picture as last time? keep adding to it
                    let key = format!("{}|{:?}|{:?}|{}|{}|{}|{}|{}", self.revision, po.camera.eye, po.camera.target, po.camera.fov, po.width, po.height, po.sun, po.bounces) + &format!("|{}|{:?}|{}|{}|{:?}|{}|{:?}|{}|{}|{:?}", po.sun_strength, po.shadow_angle, po.exposure, po.sky_strength, po.background, po.ground, po.ground_color, po.seamless_ground, po.warmth, po.area_lights) + &format!("|{:?}|{:?}|{:?}", po.sky_color, po.reinhard, po.white_balance);
                    let key = key + &format!("|{:?}|{:?}", po.camera_up, po.ortho_height);
                    let previous = self.pt.take().filter(|_| !restart);
                    let (p, img) = crate::pt::progressive(previous, key, model, &po);
                    spp = p.spp;
                    self.pt = Some(p);
                    img
                } else {
                    crate::pt::render(model, &po)
                };
                let secs = t0.elapsed().as_secs_f64();
                let data = if let Some(path) = &out {
                    if let Some(dir) = std::path::Path::new(path).parent() {
                        std::fs::create_dir_all(dir).ok();
                    }
                    img.save(path)?;
                    None
                } else {
                    use base64::Engine as _;
                    let mut buf = std::io::Cursor::new(Vec::new());
                    img.write_to(&mut buf, image::ImageFormat::Png)?;
                    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf.into_inner())))
                };
                json!({"out": out, "data": data, "seconds": secs, "spp": spp, "sun": sun_note, "embedded_lights": embedded_count, "camera": {"eye": camera.eye, "target": camera.target, "fov": camera.fov}})
            }
            Command::Render { out, opts } => {
                let whole = &self.model;
                let cam = render::camera_for(whole, &opts);
                if let Some(path) = &out {
                    if let Some(dir) = std::path::Path::new(path).parent() {
                        std::fs::create_dir_all(dir).ok();
                    }
                    // flat figures turn to face this camera
                    let turned = whole.facing(glam::DVec3::from(cam.eye));
                    let fixed = render::RenderOpts { camera: Some(cam.clone()), ..opts.clone() };
                    render::render(turned.as_ref().unwrap_or(whole), &fixed).save(path)?;
                }
                json!({"out": out, "camera": {"eye": cam.eye, "target": cam.target, "fov": cam.fov}})
            }
            Command::Environment { .. } => bail!("environment runs through Engine::exec"),
            Command::Sun { date, time, utc_offset } => serde_json::to_value(sun_at(&self.model, date.as_deref(), time.as_deref(), utc_offset)?)?,
            Command::AddLocation { lat, lon, size, cells, imagery } => {
                let p = crate::geo::add_location(m, lat, lon, size, cells, imagery, &crate::geo::fetch)?;
                json!({"group": p.group, "elevation": p.elevation, "zoom": p.zoom})
            }
            Command::Thumbnail { component, asset, path, size } => {
                use base64::Engine as _;
                let px = size.clamp(32, 512);
                let png = if let Some(c) = component {
                    crate::library::thumbnail_of(m, c, px)?
                } else if let Some(a) = asset {
                    let mut scratch = Model::default();
                    let id = crate::assets::definition(&mut scratch, &a)?;
                    crate::library::thumbnail_of(&scratch, id, px)?
                } else if let Some(p) = path {
                    crate::library::thumbnail_of_file(&p, px)?
                } else {
                    bail!("thumbnail needs a component, asset or path");
                };
                json!({"data": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))})
            }
            Command::SaveImage { path, data } => {
                use base64::Engine as _;
                let b64 = data.strip_prefix("data:image/png;base64,").ok_or_else(|| anyhow::anyhow!("expected a PNG data URL"))?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
                // only a real picture is written: decode it (this also bounds its size)
                let img = crate::safety::decode_image(&bytes, Some(image::ImageFormat::Png)).map_err(|e| anyhow::anyhow!("that isn't a PNG image the program can save: {e}"))?;
                let _ = img;
                if let Some(dir) = std::path::Path::new(&path).parent() {
                    std::fs::create_dir_all(dir).ok();
                }
                crate::safety::write_atomic(std::path::Path::new(&path), &bytes)?;
                json!({"path": path})
            }
            other => unreachable!("command routed to the wrong group: {other:?}"),
        })
    }
}
