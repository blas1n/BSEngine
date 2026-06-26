use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Current precipitation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precipitation {
    None,
    Rain,
    Snow,
    Hail,
    Sleet,
}

/// Per-scene or per-zone weather state.
/// Attach to a "world settings" entity; weather systems read this to drive
/// particle emitters, audio, post-processing, and physics drag.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weather {
    /// Cloud coverage in [0, 1]. 0 = clear sky, 1 = fully overcast.
    pub cloud_cover: f32,
    /// Wind direction and speed in world space (m/s).
    pub wind_velocity: Vec3,
    /// Precipitation type and intensity in [0, 1].
    pub precipitation: Precipitation,
    pub precipitation_intensity: f32,
    /// Ambient temperature in degrees Celsius.
    pub temperature: f32,
    /// Fog density in [0, 1]. Feeds the `Fog` component's density override.
    pub fog_density: f32,
    /// Lightning strike probability per second in [0, 1].
    pub lightning_probability: f32,
    pub enabled: bool,
}

impl Weather {
    pub fn clear() -> Self {
        Self {
            cloud_cover: 0.0,
            wind_velocity: Vec3::ZERO,
            precipitation: Precipitation::None,
            precipitation_intensity: 0.0,
            temperature: 20.0,
            fog_density: 0.0,
            lightning_probability: 0.0,
            enabled: true,
        }
    }

    pub fn stormy() -> Self {
        Self {
            cloud_cover: 1.0,
            wind_velocity: Vec3::new(8.0, 0.0, 0.0),
            precipitation: Precipitation::Rain,
            precipitation_intensity: 0.8,
            temperature: 10.0,
            fog_density: 0.3,
            lightning_probability: 0.05,
            enabled: true,
        }
    }

    pub fn blizzard() -> Self {
        Self {
            cloud_cover: 1.0,
            wind_velocity: Vec3::new(15.0, 0.0, 0.0),
            precipitation: Precipitation::Snow,
            precipitation_intensity: 1.0,
            temperature: -10.0,
            fog_density: 0.6,
            lightning_probability: 0.0,
            enabled: true,
        }
    }

    pub fn with_wind(mut self, velocity: Vec3) -> Self {
        self.wind_velocity = velocity;
        self
    }

    pub fn with_temperature(mut self, celsius: f32) -> Self {
        self.temperature = celsius;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn wind_speed(&self) -> f32 {
        self.wind_velocity.length()
    }

    pub fn is_raining(&self) -> bool {
        matches!(
            self.precipitation,
            Precipitation::Rain | Precipitation::Sleet
        ) && self.precipitation_intensity > 0.0
    }

    pub fn is_freezing(&self) -> bool {
        self.temperature <= 0.0
    }
}

impl Default for Weather {
    fn default() -> Self {
        Self::clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_clear_defaults() {
        let w = Weather::clear();
        assert_eq!(w.precipitation, Precipitation::None);
        assert!(!w.is_raining());
        assert!(!w.is_freezing());
    }

    #[test]
    fn weather_stormy_is_raining() {
        let w = Weather::stormy();
        assert!(w.is_raining());
        assert!(w.wind_speed() > 0.0);
    }

    #[test]
    fn weather_blizzard_is_freezing() {
        let w = Weather::blizzard();
        assert!(w.is_freezing());
        assert!(!w.is_raining());
    }

    #[test]
    fn weather_wind_speed() {
        let w = Weather::clear().with_wind(Vec3::new(3.0, 4.0, 0.0));
        assert!((w.wind_speed() - 5.0).abs() < 0.001);
    }

    #[test]
    fn weather_disabled() {
        let w = Weather::stormy().disabled();
        assert!(!w.enabled);
    }
}
