use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use deno_core::op2;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct SpawnParams {
    pub name: String,
    #[serde(default = "default_primitive")]
    pub primitive: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default)]
    pub rx: f32,
    #[serde(default)]
    pub ry: f32,
    #[serde(default)]
    pub rz: f32,
    #[serde(default = "default_one")]
    pub rw: f32,
    #[serde(default = "default_one")]
    pub sx: f32,
    #[serde(default = "default_one")]
    pub sy: f32,
    #[serde(default = "default_one")]
    pub sz: f32,
    pub color: Option<[f32; 3]>,
    pub emissive: Option<[f32; 3]>,
    pub script: Option<String>,
}

fn default_primitive() -> String {
    "Cube".to_string()
}
fn default_one() -> f32 {
    1.0
}

#[derive(Clone)]
pub enum ScriptCommand {
    SetTransform {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    SetRotation {
        name: String,
        rx: f32,
        ry: f32,
        rz: f32,
        rw: f32,
    },
    SetScale {
        name: String,
        sx: f32,
        sy: f32,
        sz: f32,
    },
    AddPosition {
        name: String,
        dx: f32,
        dy: f32,
        dz: f32,
    },
    AddPositionLocal {
        name: String,
        dx: f32,
        dy: f32,
        dz: f32,
    },
    SetEmissive {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetColor {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetMetallic {
        name: String,
        value: f32,
    },
    SetRoughness {
        name: String,
        value: f32,
    },
    SetPointLightColor {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetPointLightIntensity {
        name: String,
        value: f32,
    },
    SetPointLightRange {
        name: String,
        value: f32,
    },
    SetSpotLightColor {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetSpotLightIntensity {
        name: String,
        value: f32,
    },
    SetSpotLightRange {
        name: String,
        value: f32,
    },
    SetSpotLightInnerAngle {
        name: String,
        deg: f32,
    },
    SetSpotLightOuterAngle {
        name: String,
        deg: f32,
    },
    SetDirectionalLightColor {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetDirectionalLightAmbient {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetDirectionalLightDirection {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    SetCameraFov {
        name: String,
        deg: f32,
    },
    SetCameraNear {
        name: String,
        value: f32,
    },
    SetCameraFar {
        name: String,
        value: f32,
    },
    SetDamping {
        name: String,
        value: f32,
    },
    DamageShield {
        name: String,
        amount: f32,
    },
    RestoreShield {
        name: String,
        amount: f32,
    },
    SetMaxShield {
        name: String,
        value: f32,
    },
    ResetTimer {
        name: String,
    },
    SetNavDestination {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    ClearNavDestination {
        name: String,
    },
    SetNavSpeed {
        name: String,
        speed: f32,
    },
    SetNavAngularSpeed {
        name: String,
        speed: f32,
    },
    SetNavStoppingDistance {
        name: String,
        distance: f32,
    },
    SetNavEnabled {
        name: String,
        enabled: bool,
    },
    SetBloomIntensity {
        name: String,
        intensity: f32,
    },
    SetBloomThreshold {
        name: String,
        threshold: f32,
    },
    SetBloomRadius {
        name: String,
        radius: f32,
    },
    SetBloomSoftness {
        name: String,
        softness: f32,
    },
    SetBloomEnabled {
        name: String,
        enabled: bool,
    },
    SetAoRadius {
        name: String,
        radius: f32,
    },
    SetAoBias {
        name: String,
        bias: f32,
    },
    SetAoIntensity {
        name: String,
        intensity: f32,
    },
    SetAoSampleCount {
        name: String,
        count: u32,
    },
    SetAoEnabled {
        name: String,
        enabled: bool,
    },
    SetToneMapMode {
        name: String,
        mode: u32,
    },
    SetToneMapExposure {
        name: String,
        exposure: f32,
    },
    SetToneMapEnabled {
        name: String,
        enabled: bool,
    },
    SetTweenDuration {
        name: String,
        duration: f32,
    },
    SetTweenEasing {
        name: String,
        easing: u32,
    },
    SetTweenRepeat {
        name: String,
        repeat: u32,
    },
    SetTweenElapsed {
        name: String,
        elapsed: f32,
    },
    SetFollowTarget {
        name: String,
        target: String,
    },
    SetFollowOffset {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    SetFollowSpeed {
        name: String,
        speed: f32,
    },
    SetLookAtTarget {
        name: String,
        target: String,
    },
    SetLookAtUp {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    // ── Scald ────────────────────────────────────────────────────────────────
    // ── Scan ─────────────────────────────────────────────────────────────────
    // ── Scar ─────────────────────────────────────────────────────────────────
    // ── Scatter ──────────────────────────────────────────────────────────────
    // ── Scope ────────────────────────────────────────────────────────────────
    // ── Scorch ───────────────────────────────────────────────────────────────
    // ── Shear ────────────────────────────────────────────────────────────────
    // ── Shock ────────────────────────────────────────────────────────────────
    // ── Shrivel ──────────────────────────────────────────────────────────────
    // ── Shroud ───────────────────────────────────────────────────────────────
    // ── Shunt ────────────────────────────────────────────────────────────────
    // ── Silence ──────────────────────────────────────────────────────────────
    // ── Siphon ───────────────────────────────────────────────────────────────
    // ── Slam ─────────────────────────────────────────────────────────────────
    // ── Slay ─────────────────────────────────────────────────────────────────
    // ── Slide ────────────────────────────────────────────────────────────────
    // ── Slime ────────────────────────────────────────────────────────────────
    // ── Slink ────────────────────────────────────────────────────────────────
    // ── SlowMo ───────────────────────────────────────────────────────────────
    // ── Smoke ────────────────────────────────────────────────────────────────
    // ── Snare ────────────────────────────────────────────────────────────────
    // ── Soak ─────────────────────────────────────────────────────────────────
    // ── Spike ────────────────────────────────────────────────────────────────
    // ── Splinter ─────────────────────────────────────────────────────────────
    // ── Stagger ──────────────────────────────────────────────────────────────
    // ── Stake ────────────────────────────────────────────────────────────────
    // ── Stalk ────────────────────────────────────────────────────────────────
    // ── Stance ───────────────────────────────────────────────────────────────
    // ── Stat ─────────────────────────────────────────────────────────────────
    // ── Stealth ──────────────────────────────────────────────────────────────
    // ── Stomp ────────────────────────────────────────────────────────────────
    // ── Stride ───────────────────────────────────────────────────────────────
    // ── Strife ───────────────────────────────────────────────────────────────
    // ── Stumble ──────────────────────────────────────────────────────────────
    // ── Sulk ─────────────────────────────────────────────────────────────────
    // ── Sunder ───────────────────────────────────────────────────────────────
    // ── Suppress ─────────────────────────────────────────────────────────────
    // ── Surge ────────────────────────────────────────────────────────────────
    // ── Surround ─────────────────────────────────────────────────────────────
    // ── Survive ──────────────────────────────────────────────────────────────
    // ── Swim ─────────────────────────────────────────────────────────────────
    // ── Venture ───────────────────────────────────────────────────────────────
    // ── Verge ─────────────────────────────────────────────────────────────────
    // ── Verify ────────────────────────────────────────────────────────────────
    // ── Verily ────────────────────────────────────────────────────────────────
    // ── Vermin ────────────────────────────────────────────────────────────────
    // ── Vernal ────────────────────────────────────────────────────────────────
    // ── Verse ─────────────────────────────────────────────────────────────────
    // ── Vertex ────────────────────────────────────────────────────────────────
    // ── Verve ────────────────────────────────────────────────────────────────
    // ── Vest ─────────────────────────────────────────────────────────────────
    // ── Vice ─────────────────────────────────────────────────────────────────
    // ── Vim ──────────────────────────────────────────────────────────────────
    // ── Viper ────────────────────────────────────────────────────────────────
    // ── Viral ────────────────────────────────────────────────────────────────
    // ── Visit ────────────────────────────────────────────────────────────────
    // ── Vista ────────────────────────────────────────────────────────────────
    // ── Vibrate ──────────────────────────────────────────────────────────────
    // ── Viewport ─────────────────────────────────────────────────────────────
    // ── Vision ───────────────────────────────────────────────────────────────
    // ── VolumetricLight ──────────────────────────────────────────────────────
    // ── Volley ────────────────────────────────────────────────────────────────
    // ── Vortex ───────────────────────────────────────────────────────────────
    // ── Vow ──────────────────────────────────────────────────────────────────
    // ── Vulnerable ───────────────────────────────────────────────────────────
    // ── Vulture ──────────────────────────────────────────────────────────────
    // ── Wage ─────────────────────────────────────────────────────────────────
    // ── Wager ────────────────────────────────────────────────────────────────
    // ── Wail ─────────────────────────────────────────────────────────────────
    // ── Wake ─────────────────────────────────────────────────────────────────
    // ── Walk ─────────────────────────────────────────────────────────────────
    // ── Wall ─────────────────────────────────────────────────────────────────
    // ── Waltz ────────────────────────────────────────────────────────────────
    // ── Wand ─────────────────────────────────────────────────────────────────
    // ── Wane ─────────────────────────────────────────────────────────────────
    // ── Wangle ───────────────────────────────────────────────────────────────
    // ── Want ─────────────────────────────────────────────────────────────────
    // ── Wanton ───────────────────────────────────────────────────────────────
    // ── Ward ─────────────────────────────────────────────────────────────────
    // ── Warm ─────────────────────────────────────────────────────────────────
    // ── Warp ─────────────────────────────────────────────────────────────────
    // ── Wilt ─────────────────────────────────────────────────────────────────
    // ── Wily ─────────────────────────────────────────────────────────────────
    // ── Wimp ─────────────────────────────────────────────────────────────────
    // ── Wimple ───────────────────────────────────────────────────────────────
    // ── Win ──────────────────────────────────────────────────────────────────
    // ── Wince ────────────────────────────────────────────────────────────────
    // ── Winch ────────────────────────────────────────────────────────────────
    // ── Winder ───────────────────────────────────────────────────────────────
    // ── Windfall ─────────────────────────────────────────────────────────────
    // ── Windup ───────────────────────────────────────────────────────────────
    // ── Wine ─────────────────────────────────────────────────────────────────
    // ── Wing ─────────────────────────────────────────────────────────────────
    // ── Wink ─────────────────────────────────────────────────────────────────
    // ── Wino ─────────────────────────────────────────────────────────────────
    // ── Winsome ──────────────────────────────────────────────────────────────
    // ── Wintry ───────────────────────────────────────────────────────────────
    // ── Wrestle ──────────────────────────────────────────────────────────────
    // ── Wretch ───────────────────────────────────────────────────────────────
    // ── Wretched ─────────────────────────────────────────────────────────────
    // ── Wriggle ──────────────────────────────────────────────────────────────
    // ── Wring ────────────────────────────────────────────────────────────────
    // ── Wrinkle ──────────────────────────────────────────────────────────────
    // ── Wrist ────────────────────────────────────────────────────────────────
    // ── Write ────────────────────────────────────────────────────────────────
    // ── Writhe ───────────────────────────────────────────────────────────────
    // ── Wrong ────────────────────────────────────────────────────────────────
    // ── Wrongly ──────────────────────────────────────────────────────────────
    // ── Wrote ────────────────────────────────────────────────────────────────
    // ── Wroth ────────────────────────────────────────────────────────────────
    // ── Wrung ────────────────────────────────────────────────────────────────
    // ── Wry ──────────────────────────────────────────────────────────────────
    // ── Xray ─────────────────────────────────────────────────────────────────
    // ── Yak ──────────────────────────────────────────────────────────────────
    // ── Yam ──────────────────────────────────────────────────────────────────
    // ── Yang ─────────────────────────────────────────────────────────────────
    // ── Yank ─────────────────────────────────────────────────────────────────
    // ── Yap ──────────────────────────────────────────────────────────────────
    // ── Yard ─────────────────────────────────────────────────────────────────
    // ── Yare ─────────────────────────────────────────────────────────────────
    // ── Zeal ─────────────────────────────────────────────────────────────────
    // ── Zealot ───────────────────────────────────────────────────────────────
    // ── Zealotry ─────────────────────────────────────────────────────────────
    // ── Zealous ──────────────────────────────────────────────────────────────
    // ── Zeatin ───────────────────────────────────────────────────────────────
    // ── Zeaxanthin ───────────────────────────────────────────────────────────
    // ── Zebec ────────────────────────────────────────────────────────────────
    // ── Zebra ────────────────────────────────────────────────────────────────
    // ── Zebrafish ─────────────────────────────────────────────────────────────
    // ── Zebrine ───────────────────────────────────────────────────────────────
    // ── Zebroid ───────────────────────────────────────────────────────────────
    // ── Zebu ──────────────────────────────────────────────────────────────────
    // ── Zechin ────────────────────────────────────────────────────────────────
    // ── Zed ───────────────────────────────────────────────────────────────────
    // ── Zeekoe ────────────────────────────────────────────────────────────────
    // ── Zein ──────────────────────────────────────────────────────────────────
    // ── Zeitgeber ─────────────────────────────────────────────────────────────
    // ── Zeitgeist ─────────────────────────────────────────────────────────────
    // ── Zek ───────────────────────────────────────────────────────────────────
    // ── Zelkova ───────────────────────────────────────────────────────────────
    // ── Zemstvo ───────────────────────────────────────────────────────────────
    // ── Zen ───────────────────────────────────────────────────────────────────
    // ── Zenana ────────────────────────────────────────────────────────────────
    // ── Zendo ─────────────────────────────────────────────────────────────────
    // ── Zener ─────────────────────────────────────────────────────────────────
    // ── Zenith ────────────────────────────────────────────────────────────────
    // ── Zenithal ──────────────────────────────────────────────────────────────
    // ── Zeolite ───────────────────────────────────────────────────────────────
    // ── Zeolitic ──────────────────────────────────────────────────────────────
    // ── Zephyr ────────────────────────────────────────────────────────────────
    // ── Zeppelin ──────────────────────────────────────────────────────────────
    // ── Zerk ──────────────────────────────────────────────────────────────────
    // ── Zeroth ───────────────────────────────────────────────────────────────
    // ── Zester ───────────────────────────────────────────────────────────────
    // ── Zestful ──────────────────────────────────────────────────────────────
    // ── Zeta ─────────────────────────────────────────────────────────────────
    // ── Zetetic ──────────────────────────────────────────────────────────────
    // ── Zeugen ───────────────────────────────────────────────────────────────
    // ── Zeugma ───────────────────────────────────────────────────────────────
    // ── Zho ──────────────────────────────────────────────────────────────────
    // ── Zing ─────────────────────────────────────────────────────────────────
    // ── Zinger ───────────────────────────────────────────────────────────────
    // ── Zink ─────────────────────────────────────────────────────────────────
    // ── Zinnia ───────────────────────────────────────────────────────────────
    // ── Zip ──────────────────────────────────────────────────────────────────
    // ── Zipper ───────────────────────────────────────────────────────────────
    // ── Zippier ──────────────────────────────────────────────────────────────
    // ── Zippy ────────────────────────────────────────────────────────────────
    // ── Quest ────────────────────────────────────────────────────────────────
    // ── Radar ────────────────────────────────────────────────────────────────
    // ── Rage ─────────────────────────────────────────────────────────────────
    // ── Rally ────────────────────────────────────────────────────────────────
    // ── Rampage ──────────────────────────────────────────────────────────────
    // ── Ravage ───────────────────────────────────────────────────────────────
    // ── Reave ────────────────────────────────────────────────────────────────
    // ── Rebound ──────────────────────────────────────────────────────────────
    // ── Recharge ─────────────────────────────────────────────────────────────
    // ── Reckless ─────────────────────────────────────────────────────────────
    // ── Recluse ──────────────────────────────────────────────────────────────
    // ── Recoil ───────────────────────────────────────────────────────────────
    // ── Reflect ──────────────────────────────────────────────────────────────
    // ── Reflex ───────────────────────────────────────────────────────────────
    // ── Repel ────────────────────────────────────────────────────────────────
    // ── Repose ───────────────────────────────────────────────────────────────
    // ── Respawn ──────────────────────────────────────────────────────────────
    // ── Retaliate ────────────────────────────────────────────────────────────
    // ── Revenge ──────────────────────────────────────────────────────────────
    // ── Reveal ───────────────────────────────────────────────────────────────
    // ── Revive ───────────────────────────────────────────────────────────────
    // ── Ricochet ─────────────────────────────────────────────────────────────
    // ── Rifle ────────────────────────────────────────────────────────────────
    // ── Rot ──────────────────────────────────────────────────────────────────
    // ── Rout ─────────────────────────────────────────────────────────────────
    // ── Rupture ──────────────────────────────────────────────────────────────
    PlayAnimation {
        name: String,
        clip: String,
    },
    PauseAnimation {
        name: String,
    },
    ResumeAnimation {
        name: String,
    },
    ResetAnimation {
        name: String,
    },
    SetAnimationSpeed {
        name: String,
        speed: f32,
    },
    SetAnimationLooping {
        name: String,
        looping: bool,
    },
    AsmSetTrigger {
        name: String,
        trigger: String,
    },
    AsmSetFloat {
        name: String,
        param: String,
        value: f32,
    },
    AsmSetBool {
        name: String,
        param: String,
        value: bool,
    },
    SetLifetime {
        name: String,
        seconds: f32,
    },
    Spawn(SpawnParams),
    Destroy {
        name: String,
    },
    SetVisible {
        name: String,
        visible: bool,
    },
    PlaySound {
        id: u32,
        path: String,
        volume: f32,
        loop_: bool,
    },
    StopSound {
        id: u32,
    },
    PauseSound {
        id: u32,
    },
    ResumeSound {
        id: u32,
    },
    SetSoundVolume {
        id: u32,
        db: f32,
    },
    SetSoundPanning {
        id: u32,
        panning: f32,
    },
    SetSoundPlaybackRate {
        id: u32,
        rate: f32,
    },
    SeekSound {
        id: u32,
        position: f64,
    },
    SetHudText {
        id: String,
        text: String,
    },
    ClearHudText {
        id: String,
    },
    SetUiLabel {
        id: String,
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
    },
    SetUiButton {
        id: String,
        label: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    SetUiPanel {
        id: String,
        title: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    SetUiTextInput {
        id: String,
        hint: String,
        x: f32,
        y: f32,
        width: f32,
    },
    RemoveUiWidget {
        id: String,
    },
    ClearUiWidgets,
    LoadScene {
        path: String,
    },
    SetSkybox {
        path: String,
    },
    SetParent {
        child: String,
        parent: String,
    },
    ClearParent {
        child: String,
    },
    SetCursorVisible {
        visible: bool,
    },
    SetCursorLocked {
        locked: bool,
    },
    AddImpulse {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
    },
    AddImpulseAtPoint {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
        px: f32,
        py: f32,
        pz: f32,
    },
    AddForce {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
    },
    AddForceAtPoint {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
        px: f32,
        py: f32,
        pz: f32,
    },
    ResetForces {
        name: String,
    },
    SetVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetVelocityX {
        name: String,
        vx: f32,
    },
    SetVelocityY {
        name: String,
        vy: f32,
    },
    SetVelocityZ {
        name: String,
        vz: f32,
    },
    SetGravity {
        magnitude: f32,
    },
    SetAngularVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetAngularVelocityX {
        name: String,
        vx: f32,
    },
    SetAngularVelocityY {
        name: String,
        vy: f32,
    },
    SetAngularVelocityZ {
        name: String,
        vz: f32,
    },
    AddVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    AddAngularVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    AddAngularImpulse {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    AddTorque {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetCCDEnabled {
        name: String,
        enabled: bool,
    },
    SetLinearDamping {
        name: String,
        damping: f32,
    },
    SetAngularDamping {
        name: String,
        damping: f32,
    },
    SetMass {
        name: String,
        mass: f32,
    },
    SetKinematic {
        name: String,
        kinematic: bool,
    },
    SetGravityScale {
        name: String,
        scale: f32,
    },
    SetColliderSensor {
        name: String,
        sensor: bool,
    },
    SetRestitution {
        name: String,
        restitution: f32,
    },
    SetFriction {
        name: String,
        friction: f32,
    },
    LockRotation {
        name: String,
        lock_x: bool,
        lock_y: bool,
        lock_z: bool,
    },
    LockTranslation {
        name: String,
        lock_x: bool,
        lock_y: bool,
        lock_z: bool,
    },
    WakeUp {
        name: String,
    },
    PutToSleep {
        name: String,
    },
    SetPositionX {
        name: String,
        x: f32,
    },
    SetPositionY {
        name: String,
        y: f32,
    },
    SetPositionZ {
        name: String,
        z: f32,
    },
    AddPositionX {
        name: String,
        dx: f32,
    },
    AddPositionY {
        name: String,
        dy: f32,
    },
    AddPositionZ {
        name: String,
        dz: f32,
    },
    RotateBy {
        name: String,
        rx: f32,
        ry: f32,
        rz: f32,
        rw: f32,
    },
    RotateAroundAxis {
        name: String,
        ax: f32,
        ay: f32,
        az: f32,
        angle_deg: f32,
    },
    AddRotationEuler {
        name: String,
        pitch: f32,
        yaw: f32,
        roll: f32,
    },
    AddRotationEulerX {
        name: String,
        deg: f32,
    },
    AddRotationEulerY {
        name: String,
        deg: f32,
    },
    AddRotationEulerZ {
        name: String,
        deg: f32,
    },
    SetScaleX {
        name: String,
        x: f32,
    },
    SetScaleY {
        name: String,
        y: f32,
    },
    SetScaleZ {
        name: String,
        z: f32,
    },
    AddScaleX {
        name: String,
        dx: f32,
    },
    AddScaleY {
        name: String,
        dy: f32,
    },
    AddScaleZ {
        name: String,
        dz: f32,
    },
    SetRotationEuler {
        name: String,
        pitch_deg: f32,
        yaw_deg: f32,
        roll_deg: f32,
    },
    AddScale {
        name: String,
        sx: f32,
        sy: f32,
        sz: f32,
    },
    SetRotationEulerX {
        name: String,
        deg: f32,
    },
    SetRotationEulerY {
        name: String,
        deg: f32,
    },
    SetRotationEulerZ {
        name: String,
        deg: f32,
    },
    MultiplyScale {
        name: String,
        sx: f32,
        sy: f32,
        sz: f32,
    },
    NavmeshInit {
        width: u32,
        depth: u32,
        cell_size: f32,
        origin_x: f32,
        origin_y: f32,
        origin_z: f32,
    },
    NavmeshSetWalkable {
        x: u32,
        z: u32,
        walkable: bool,
    },
    SaveGame {
        path: String,
    },
    LoadGame {
        path: String,
    },
    SetCustomShader {
        name: String,
        path: String,
    },
    ClearCustomShader {
        name: String,
    },
    NetworkStartServer {
        port: u16,
    },
    NetworkConnect {
        host: String,
        port: u16,
    },
    NetworkDisconnect,
}

thread_local! {
    pub(crate) static TRANSFORM_SNAPSHOT: RefCell<HashMap<String, (Vec3, Quat, Vec3)>> =
        RefCell::new(HashMap::new());
    pub(crate) static WORLD_TRANSFORM_SNAPSHOT: RefCell<HashMap<String, (Vec3, Quat, Vec3)>> =
        RefCell::new(HashMap::new());
    pub(crate) static KEY_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static KEY_JUST_PRESSED_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static KEY_JUST_RELEASED_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static ENTITY_NAMES_SNAPSHOT: RefCell<Vec<String>> =
        const { RefCell::new(Vec::new()) };
    pub(crate) static COLLISION_SNAPSHOT: RefCell<Vec<(String, String, bool)>> =
        const { RefCell::new(Vec::new()) };
    pub(crate) static COMMAND_BUFFER: RefCell<Vec<ScriptCommand>> =
        const { RefCell::new(Vec::new()) };
    pub(crate) static SOUND_ID_COUNTER: RefCell<u32> =
        const { RefCell::new(0) };

    // Mouse state snapshots (bit 0=Left, bit 1=Right, bit 2=Middle)
    pub(crate) static MOUSE_PRESSED_SNAPSHOT: RefCell<u8> = const { RefCell::new(0) };
    pub(crate) static MOUSE_JUST_PRESSED_SNAPSHOT: RefCell<u8> = const { RefCell::new(0) };
    pub(crate) static MOUSE_JUST_RELEASED_SNAPSHOT: RefCell<u8> = const { RefCell::new(0) };
    pub(crate) static MOUSE_POS_SNAPSHOT: RefCell<(f64, f64)> = const { RefCell::new((0.0, 0.0)) };
    pub(crate) static MOUSE_DELTA_SNAPSHOT: RefCell<(f64, f64)> = const { RefCell::new((0.0, 0.0)) };

    // UI widget click state — button IDs clicked this frame
    pub(crate) static UI_CLICKED_SNAPSHOT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };

    // Raycast: raw pointer to PhysicsWorld, valid only during V8 execution in run_scripts.
    // Safety: set before V8 runs, cleared immediately after. V8 is synchronous.
    pub(crate) static PHYSICS_WORLD_PTR: RefCell<*const bsengine_physics::PhysicsWorld> =
        const { RefCell::new(std::ptr::null()) };

    // Entity name lookup for raycast results: entity.to_bits() → name
    pub(crate) static ENTITY_NAME_MAP: RefCell<HashMap<u64, String>> =
        RefCell::new(HashMap::new());

    // Gamepad button state (bit 0=South..15=DPadRight)
    pub(crate) static GAMEPAD_BUTTON_SNAPSHOT: RefCell<u16> = const { RefCell::new(0) };
    pub(crate) static GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT: RefCell<u16> = const { RefCell::new(0) };
    pub(crate) static GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT: RefCell<u16> = const { RefCell::new(0) };
    // (left_x, left_y, right_x, right_y, left_trigger, right_trigger)
    pub(crate) static GAMEPAD_STICKS_SNAPSHOT: RefCell<(f32, f32, f32, f32, f32, f32)> =
        const { RefCell::new((0.0, 0.0, 0.0, 0.0, 0.0, 0.0)) };

    pub(crate) static VISIBLE_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → base_color [r, g, b] (only for entities with a Material component)
    pub(crate) static MATERIAL_COLOR_SNAPSHOT: RefCell<HashMap<String, [f32; 3]>> =
        RefCell::new(HashMap::new());

    // name → emissive [r, g, b] (only for entities with a Material component)
    pub(crate) static MATERIAL_EMISSIVE_SNAPSHOT: RefCell<HashMap<String, [f32; 3]>> =
        RefCell::new(HashMap::new());

    // name → metallic (only for entities with a Material component)
    pub(crate) static MATERIAL_METALLIC_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → roughness (only for entities with a Material component)
    pub(crate) static MATERIAL_ROUGHNESS_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    pub(crate) static TIME_ELAPSED_SNAPSHOT: RefCell<f32> = const { RefCell::new(0.0) };
    pub(crate) static TIME_DELTA_SNAPSHOT: RefCell<f32> = const { RefCell::new(0.0) };

    pub(crate) static SCREEN_SIZE_SNAPSHOT: RefCell<(u32, u32)> = const { RefCell::new((1280, 720)) };

    // name → linear velocity Vec3 (only for entities with a physics body)
    pub(crate) static VELOCITY_SNAPSHOT: RefCell<HashMap<String, Vec3>> =
        RefCell::new(HashMap::new());

    pub(crate) static GRAVITY_SNAPSHOT: RefCell<f32> = const { RefCell::new(9.81) };

    // name → angular velocity Vec3 (only for entities with a physics body)
    pub(crate) static ANGULAR_VELOCITY_SNAPSHOT: RefCell<HashMap<String, Vec3>> =
        RefCell::new(HashMap::new());

    // name → mass (only for entities with a physics body)
    pub(crate) static MASS_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → gravity scale (only for entities with a physics body)
    pub(crate) static GRAVITY_SCALE_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → is_kinematic (only for entities with a physics body)
    pub(crate) static BODY_TYPE_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → is_sensor (only for entities with at least one collider)
    pub(crate) static COLLIDER_SENSOR_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → is_sleeping (only for entities with a physics body)
    pub(crate) static SLEEP_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → linear damping (only for entities with a physics body)
    pub(crate) static LINEAR_DAMPING_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → angular damping (only for entities with a physics body)
    pub(crate) static ANGULAR_DAMPING_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → restitution (only for entities with at least one collider)
    pub(crate) static RESTITUTION_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → friction (only for entities with at least one collider)
    pub(crate) static FRICTION_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // child_name → parent_name (only for entities that have a Parent component)
    pub(crate) static PARENT_SNAPSHOT: RefCell<HashMap<String, String>> =
        RefCell::new(HashMap::new());
    // parent_name → [child_names]
    pub(crate) static CHILDREN_SNAPSHOT: RefCell<HashMap<String, Vec<String>>> =
        RefCell::new(HashMap::new());

    // tag label → [entity names]

    // entity name → [tag labels]

    // sound id → playback state string ("playing", "pausing", "paused", etc.)
    pub(crate) static SOUND_STATE_SNAPSHOT: RefCell<HashMap<u32, String>> =
        RefCell::new(HashMap::new());

    // sound id → playback position in seconds
    pub(crate) static SOUND_POSITION_SNAPSHOT: RefCell<HashMap<u32, f64>> =
        RefCell::new(HashMap::new());

    // entity name → (current_health, max_health)

    // entity name → (clip, time, speed, looping, playing)
    pub(crate) static ANIMATION_SNAPSHOT: RefCell<HashMap<String, (String, f32, f32, bool, bool)>> =
        RefCell::new(HashMap::new());

    pub(crate) static ASM_STATE_SNAPSHOT: RefCell<HashMap<String, String>> =
        RefCell::new(HashMap::new());

    // entity name → remaining lifetime seconds
    pub(crate) static LIFETIME_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // entity name → (current, max, exhausted)

    // entity name → (current, max)

    // entity name → (base, effective)

    // entity name → (heat_stacks, max_stacks, amplify_per_stack, stack_duration, just_scalded, just_cooled, enabled)
    // entity name → (radius, interval, timer, just_pulsed, enabled)
    // entity name → (scars, max_scars, regen_penalty_per_scar, just_scarred, just_cleansed, enabled)
    // entity name → (duration, timer, spread_multiplier, extra_pellets, just_scattered, just_cleared, enabled)
    // entity name → (active, accuracy_bonus, range_bonus, move_speed_penalty, just_scoped, just_unscoped, enabled)
    // entity name → (duration, timer, fire_amplify, dot_rate, just_scorched, just_healed, enabled)
    // entity name → (armor_penetration, flat_penetration, enabled)
    // entity name → (duration, timer, damage_per_second, interrupt_chance, just_shocked, just_discharged, enabled)
    // entity name → (shrivel_fraction, shrivel_rate, recovery_rate, shrivel_factor, shriveled, just_afflicted, just_recovered, enabled)
    // entity name → (charges, save_health_fraction, cooldown, cooldown_timer, just_saved, just_exhausted, enabled)
    // entity name → (shunt_resistance, last_shunt_magnitude, shunts_received, cooldown_timer, cooldown, just_shunted, enabled)
    // entity name → (duration, timer, just_silenced, just_unsilenced, enabled)
    // entity name → (duration, timer, drain_per_second, return_fraction, just_started, just_ended, enabled)
    // entity name → (phase, slam_speed, impact_radius, impact_force, min_height, launch_height, recovery_time, recovery_timer, cooldown, cooldown_timer, wants_slam, enabled)
    // entity name → (kill_count, threshold, trigger_count, just_triggered, enabled)
    // entity name → (phase, dir_x, dir_y, dir_z, duration, brake_start, elapsed, slide_speed, wants_slide, crouch_scale, enabled)
    // entity name → (slime_timer, slow_factor, slimed, just_slimed, just_cleansed, enabled)
    // entity name → (active, speed_reduction, noise_reduction, just_engaged, just_disengaged, enabled)
    // entity name → (target_scale, current_scale, blend_speed, max_duration, elapsed, charge, drain_rate, source, enabled)
    // entity name → (style, rate, color_r, color_g, color_b, color_a, particle_speed, spread_rate, particle_lifetime, offset_x, offset_y, offset_z, burst_duration, elapsed, enabled)
    // entity name → (duration, timer, slow_fraction, escape_chance, just_snared, just_escaped, enabled)
    // entity name → (soak_level, decay_rate, fire_resistance, lightning_amplify, just_soaked, just_dried, enabled)
    // entity name → (duration, timer, damage, push_force, just_extended, just_retracted, enabled)
    // entity name → (threshold, radius, damage_fraction, cooldown, cooldown_timer, just_splintered, enabled)
    // entity name → (phase, stagger_threshold, stagger_duration, stagger_timer, recovery_duration, recovery_timer, stagger_count, resist, just_staggered, enabled)
    // entity name → (active, hold_timer, min_hold, damage_bonus, just_staked, just_broke, enabled)
    // entity name → (active, damage_multiplier, just_began, just_consumed, enabled)
    // entity name → (current, offense_bonus, defense_reduction, just_changed, enabled)
    // entity name → (base, bonus, multiplier, min, max)
    // entity name → (base_visibility, visibility_modifier, noise_level, noise_decay_rate, sneaking, sneak_visibility_scale, enabled)
    // entity name → (magnitude, impact_radius, damage_per_unit, just_stomped, enabled)
    // entity name → (stride_count, max_strides, speed_bonus, just_peaked, just_broke, enabled)
    // entity name → (strife, max_strife, gain_per_hit, decay_rate, just_peaked, enabled)
    // entity name → (stumble_timer, stumble_duration, vulnerability_factor, move_penalty, stumble_count, stumbling, just_stumbled, just_recovered, enabled)
    // entity name → (sulk_depth, sulk_rate, recovery_rate, support_penalty, sulking, just_sulked, just_snapped_out, enabled)
    // entity name → (shards, max_shards, damage_reduction_per_shard, just_sundered, enabled)
    // entity name → (duration, timer, potency_fraction, blocks_ultimates, just_suppressed, just_lifted, enabled)
    // entity name → (duration, timer, multiplier, just_surged, just_expired, enabled)
    // entity name → (adjacent_count, encircle_threshold, defense_bonus, just_encircled, just_cleared, enabled)
    // entity name → (charges, max_charges, just_survived, enabled)
    // entity name → (state, swim_speed, dive_speed, ascent_speed, breath_remaining, max_breath, breath_drain_rate, breath_regen_rate, depth, submerge_depth, wants_dive, wants_surface, enabled)
    // xray_range, pulse_bonus, pulse_timer, pulse_duration, just_pulsed, enabled
    // yak_interval, elapsed, silence_remaining, just_yakked, just_silenced, enabled
    // yield_stored, yield_cap, yield_rate, just_capped, enabled
    // charge, threshold, polarity, just_flipped, enabled
    // impulse, peak, just_yanked, enabled
    // yap_interval, cooldown_remaining, just_yapped, enabled
    // range, intruder_count_as_f32, just_breached, just_cleared, enabled
    // readiness, max_readiness, recovery_rate, just_primed, just_exhausted, enabled
    // yarn: length, max_length, just_snagged, just_rewound, enabled
    // yaw: heading, turn_rate, just_rotated, enabled
    // yawl: phase, cycle_count, just_toggled, just_cycled, enabled
    // yawn: yawn_threshold, idle_time, just_yawned, is_drowsy, enabled
    // yawp: volume, max_volume, decay_rate, just_shrieked, just_silenced, enabled
    // yay: elation, max_elation, decay_rate, just_peaked, just_faded, enabled
    // yea: count, required, just_passed, just_revoked, enabled
    // year: elapsed, period, just_cycled, just_new_season, enabled
    // yearn: longing, max_longing, accumulation_rate, just_fulfilled, just_suppressed, enabled
    // yeast: quantity, max_quantity, growth_rate, just_peaked, just_dormant, enabled
    // yell: charge_level, max_charge, charge_rate, power_bonus, charging, just_shouted, enabled
    // yelp: yelps, max_yelps, decay_timer, decay_interval, just_yelped, just_peaked, just_calmed, enabled
    // yen: yen_level, max_yen, yen_rate, just_yearned, just_satisfied, enabled
    // yenta: gossip, max_gossip, fade_rate, just_notorious, just_forgotten, enabled
    // yeoman: reliability, max_reliability, falter_rate, served, just_trusted, just_failed, enabled
    // yep: consent, max_consent, doubt_rate, just_agreed, just_withdrew, enabled
    // yes: yes_count, yes_threshold, just_reached, just_reset, enabled
    // yeti: tension, max_tension, fade_rate, just_manifested, just_fled, enabled
    // yew: exposure, max_exposure, just_saturated, enabled
    // yield: yield_duration, yield_remaining, is_yielding, just_started_yielding, just_finished_yielding, enabled
    // yikes: fright, max_fright, calm_rate, just_startled, just_calmed, enabled
    // yin: balance, just_darkened, just_lightened, enabled
    // yip: burst_limit, burst_count, cooldown, cooldown_remaining, just_yipped, just_burst_out, enabled
    // yips: pressure, max_pressure, just_seized, just_composed, enabled
    // yodel: echo_delay, echo_remaining, awaiting_echo, just_yodeled, just_echoed, enabled
    // yoga: flexibility, max_flexibility, recovery_rate, just_centered, just_broken, enabled
    // yogi: depth, max_depth, focus_rate, drift_rate, meditating, just_transcended, just_scattered, enabled
    // yoke: yoke_weight, max_weight, recovery_rate, speed_penalty, just_burdened, just_freed, enabled
    // yokel: credulity, max_credulity, wisdom_rate, just_fooled, just_savvy, enabled
    // yolk: fragility, max_fragility, damage_multiplier, just_cracked, enabled
    // yonder: distance, max_range, just_acquired, just_arrived, enabled
    // yore: current_value, peak_value, just_peaked, enabled
    // yule: warmth, max_warmth, heat_rate, cool_rate, is_heating, just_peaked, just_frosted, enabled
    // yum: fullness, max_fullness, hunger_rate, just_sated, just_starved, enabled
    // yummy: appeal, max_appeal, spoil_rate, just_irresistible, just_spoiled, enabled
    // yup: streak (as f32), max_streak (as f32), responded, just_peaked, just_broke, enabled
    // yurt: durability, max_durability, deployed, just_deployed, just_collapsed, enabled
    // zafu: support, max_support, recovery_rate, just_supported, just_exhausted, enabled
    // zag: offset, max_offset, drift_rate, just_peaked, just_bottomed, enabled
    // zaibatsu: reach, max_reach, consolidate_rate, just_dominant, just_dissolved, enabled
    // zakat: tithe, max_tithe, accrue_rate, just_fulfilled, just_discharged, enabled
    // zamia: frond, max_frond, grow_rate, just_spread, just_crozier, enabled
    // zanily: verve, max_verve, jest_rate, just_capered, just_deadpan, enabled
    // zaniness: mirth, max_mirth, caprice_rate, just_clowned, just_sobered, enabled
    // zany: whimsy, max_whimsy, fade_rate, just_unhinged, just_sane, enabled
    // zap: zap_power, zap_range, cooldown_duration, cooldown_timer, just_zapped, enabled
    // zapper: charge, max_charge, charge_rate, just_zapped, just_discharged, enabled
    // zappy: vitality, max_vitality, spark_rate, just_sparked, just_fizzled, enabled
    // zeal: zeal_level, max_zeal, threshold, decay_rate, just_devoted, just_lapsed, enabled
    // zealot: fervor, max_fervor, zeal_rate, just_zealous, just_lapsed, enabled
    // zealotry: fervor, max_fervor, dogma_rate, just_fanatical, just_lapsed, enabled
    // zealous: conviction, max_conviction, devote_rate, just_zealous, just_wavered, enabled
    // zeatin: division, max_division, proliferate_rate, just_proliferating, just_arrested, enabled
    // zeaxanthin: pigment, max_pigment, absorb_rate, just_saturated, just_depleted, enabled
    // zebec: bearing, max_bearing, drift_rate, just_on_course, just_adrift, enabled
    // zebra: phase, stripe_width, speed, dark_stripe, just_switched, enabled
    // zebrafish: transparency, max_transparency, develop_rate, just_transparent, just_opaque, enabled
    // zebrine: striping, max_striping, pattern_rate, just_banded, just_blank, enabled
    // zebroid: hybridization, max_hybridization, blend_rate, just_hybrid, just_pure, enabled
    // zebu: burden, max_burden, strain_rate, just_overwhelmed, just_unburdened, enabled
    // zechin: gold, max_gold, earn_rate, just_wealthy, just_bankrupt, enabled
    // zed: fatigue, max_fatigue, wind_down_rate, just_spent, just_rallied, enabled
    // zeekoe: territory, max_territory, wallow_rate, just_dominant, just_displaced, enabled
    // zein: protein, max_protein, store_rate, just_loaded, just_depleted, enabled
    // zeitgeber: entrainment, max_entrainment, cue_rate, just_entrained, just_drifted, enabled
    // zeitgeist: momentum, max_momentum, fade_rate, just_surged, just_faded, enabled
    // zek: output, max_output, toil_rate, just_fulfilled, just_exhausted, enabled
    // zelkova: canopy, max_canopy, grow_rate, just_canopied, just_bare, enabled
    // zemstvo: authority, max_authority, mandate_rate, just_empowered, just_abolished, enabled
    // zen: zen_level, max_zen, restore_rate, just_achieved, just_broken, enabled
    // zenana: solace, max_solace, shelter_rate, just_secluded, just_disturbed, enabled
    // zendo: harmony, max_harmony, attune_rate, just_harmonized, just_discordant, enabled
    // zener: voltage, max_voltage, charge_rate, just_critical, just_depleted, enabled
    // zenith: altitude, max_altitude, descent_rate, just_peaked, just_grounded, enabled
    // zenithal: elevation, max_elevation, rise_rate, just_peaked, just_nadir, enabled
    // zeolite: purity, max_purity, cleanse_rate, just_cleansed, just_fouled, enabled
    // zeolitic: microporosity, max_microporosity, sieve_rate, just_ordered, just_amorphous, enabled
    // zephyr: gust, max_gust, calm_rate, just_surged, just_stilled, enabled
    // zeppelin: lift, max_lift, leak_rate, just_aloft, just_grounded, enabled
    // zerk: lubrication, max_lubrication, flow_rate, just_serviced, just_seized, enabled
    // zeroth: deviation, max_deviation, drift_rate, just_saturated, just_grounded, enabled
    // zester: scraped, max_scraped, grate_rate, just_zested, just_depleted, enabled
    // zestful: energy, max_energy, invigorate_rate, just_zestful, just_listless, enabled
    // zeta: potential, max_potential, flux_rate, just_critical, just_neutral, enabled
    // zetetic: inquiry, max_inquiry, probe_rate, just_resolved, just_abandoned, enabled
    // zeugen: prominence, max_prominence, denude_rate, just_prominent, just_eroded, enabled
    // zeugma: tension, max_tension, tighten_rate, just_yoked, just_severed, enabled
    // zho: vigor, max_vigor, cross_rate, just_thriving, just_exhausted, enabled
    // zibet: scent, max_scent, mark_rate, just_marked, just_faded, enabled
    // zidovudine: dose, max_dose, infusion_rate, just_suppressed, just_cleared, enabled
    // ziggurat: tier, max_tier, rise_rate, just_crowned, just_razed, enabled
    // zigzag: phase, period, speed, rising, just_reversed, enabled
    // zikr: resonance, max_resonance, hum_rate, just_resonant, just_hushed, enabled
    // zilch: deprivation, max_deprivation, replenish_rate, just_exhausted, just_sated, enabled
    // zileuton: coverage, max_coverage, absorption_rate, just_covered, just_lapsed, enabled
    // zill: ring, max_ring, sustain_rate, just_ringing, just_damped, enabled
    // zillion: count, max_count, tally_rate, just_astronomical, just_zeroed, enabled
    // zimb: swarm, max_swarm, swarm_rate, just_swarming, just_dispersed, enabled
    // zinc: mineral, max_mineral, metabolic_rate, just_optimal, just_deficient, enabled
    // zincate: concentration, max_concentration, deposit_rate, just_saturated, just_depleted, enabled
    // zincite: saturation, max_saturation, deposit_rate, just_crystallized, just_depleted, enabled
    // zine: content, max_content, draft_rate, just_published, just_blank, enabled
    // zineb: residue, max_residue, spray_rate, just_protected, just_exposed, enabled
    // zinfandel: ferment, max_ferment, mature_rate, just_matured, just_racked, enabled
    // zing: zing_charge, zing_threshold, zing_count, just_zinged, false, enabled
    // zinger: sting, max_sting, sharpen_rate, just_stinging, just_soothed, enabled
    // zink: breath, max_breath, breath_rate, just_resonant, just_breathless, enabled
    // zinnia: petals, max_petals, bloom_rate, just_blooming, just_wilted, enabled
    // zip: zip_charge, max_charge, drain_rate, just_activated, just_exhausted, enabled
    // zipper: closure, max_closure, slide_rate, just_sealed, just_open, enabled
    // zippier: pace, max_pace, sprint_rate, just_fastest, just_stalled, enabled
    // zippy: pep, max_pep, perk_rate, just_peppy, just_tired, enabled
    // zircon: hardness, max_hardness, harden_rate, just_flawless, just_fractured, enabled
    // zirconia: hardness, max_hardness, sinter_rate, just_sintered, just_cracked, enabled
    // zirconium: purity, max_purity, refine_rate, just_refined, just_tarnished, enabled
    // zit: zit_count as f32, max_zits as f32, 0.0 (placeholder), just_inflamed, just_cleared, enabled
    // zither: resonance, max_resonance, decay_rate, just_harmonized, just_silenced, enabled
    // ziti: heat_level, max_heat, simmer_rate, just_scorching, just_raw, enabled
    // zoanthropy: delusion, max_delusion, inhabit_rate, just_feral, just_lucid, enabled
    // zodiac: alignment, max_alignment, transit_rate, just_aligned, just_voided, enabled
    // zodiacal: alignment, max_alignment, aspect_rate, just_aligned, just_discordant, enabled
    // zoea: molt, max_molt, grow_rate, just_metamorphosed, just_shed, enabled
    // zoecium: colony, max_colony, encrust_rate, just_established, just_dispersed, enabled
    // zoetic: vitality, max_vitality, quicken_rate, just_vital, just_dormant, enabled
    // zoetrope: frame, max_frame, spin_rate, just_cycling, just_stalled, enabled
    // zoic: vitality, max_vitality, fauna_rate, just_teeming, just_barren, enabled
    // zokor: burrow, max_burrow, tunnel_rate, just_entrenched, just_collapsed, enabled
    // zombie: shamble, max_shamble, rot_rate, just_risen, just_decayed, enabled
    // zombify: taint, max_taint, spread_rate, just_zombified, just_purified, enabled
    // zonal: coverage, max_coverage, zone_rate, just_zoned, just_unzoned, enabled
    // zonate: zonation, max_zonation, band_rate, just_banded, just_dispersed, enabled
    // zonation: distribution, max_distribution, layer_rate, just_stratified, just_collapsed, enabled
    // zone: zone_control, max_zone, advance_rate, just_captured, just_lost, enabled
    // zoner: control, max_control, expand_rate, just_dominant, just_yielded, enabled
    // zoning: influence, max_influence, spread_rate, just_dominant, just_neutral, enabled
    // zonk: daze, max_daze, bonk_rate, just_knocked_out, just_cleared, enabled
    // zoo: population, max_population, breed_rate, just_full, just_empty, enabled
    // zoogenous: genesis, max_genesis, origin_rate, just_emerged, just_inert, enabled
    // zoogeography: range, max_range, survey_rate, just_mapped, just_uncharted, enabled
    // zooglea: biofilm, max_biofilm, culture_rate, just_encrusted, just_dispersed, enabled
    // zoography: survey, max_survey, map_rate, just_mapped, just_void, enabled
    // zooid: integration, max_integration, bud_rate, just_integrated, just_isolated, enabled
    // zookeeper: care, max_care, tend_rate, just_thriving, just_neglected, enabled
    // zoolatry: devotion, max_devotion, revere_rate, just_revered, just_profaned, enabled
    // zoological: catalogue, max_catalogue, accession_rate, just_complete, just_depleted, enabled
    // zoologist: expertise, max_expertise, study_rate, just_mastered, just_lapsed, enabled
    // zoology: specimens, max_specimens, catalog_rate, just_cataloged, just_extinct, enabled
    // zoom: zoom_steps as f32, max_steps as f32, 0.0, just_stepped_in, just_maxed, enabled
    // zoometry: measurement, max_measurement, calibrate_rate, just_calibrated, just_uncalibrated, enabled
    // zoomorph: morph, max_morph, flux_rate, just_shifted, just_reverted, enabled
    // zoomorphic: iconography, max_iconography, depict_rate, just_engraved, just_eroded, enabled
    // zoomorphism: expression, max_expression, channel_rate, just_manifested, just_lapsed, enabled
    // zoonosis: contagion, max_contagion, transmit_rate, just_spread, just_contained, enabled
    // zoonotic: spillover, max_spillover, transmit_rate, just_emerged, just_contained, enabled
    // zoophagous: feeding, max_feeding, prey_rate, just_satiated, just_famished, enabled
    // zoophile: affection, max_affection, empathy_rate, just_bonded, just_estranged, enabled
    // zoophilia: affinity, max_affinity, bond_rate, just_bonded, just_estranged, enabled
    // zoophilous: affinity, max_affinity, attract_rate, just_adapted, just_repelled, enabled
    // zoophily: pollination, max_pollination, visit_rate, just_fertilized, just_barren, enabled
    // zoophyte: vitality, max_vitality, polyp_rate, just_flourished, just_withered, enabled
    // zooplankton: drift, max_drift, bloom_rate, just_bloomed, just_clear, enabled
    // zoosphere: fauna, max_fauna, colonize_rate, just_saturated, just_empty, enabled
    // zoosperm: motility, max_motility, swim_rate, just_fertile, just_sterile, enabled
    // zoospore: motility, max_motility, swim_rate, just_dispersed, just_settled, enabled
    // zootechnics: husbandry, max_husbandry, tend_rate, just_thriving, just_declining, enabled
    // zootomy: dissection, max_dissection, study_rate, just_complete, just_spoiled, enabled
    // zooxanthella: symbiosis, max_symbiosis, photosynthesize_rate, just_thriving, just_bleached, enabled
    // zoogamy: gamete, max_gamete, mate_rate, just_mated, just_sterile, enabled
    // zoogenesis: genesis, max_genesis, emerge_rate, just_emerged, just_extinct, enabled
    // zoognomy: lore, max_lore, study_rate, just_learned, just_forgotten, enabled
    // zoopathology: lesion, max_lesion, infect_rate, just_afflicted, just_cured, enabled
    // zoophobia: dread, max_dread, fear_rate, just_panicked, just_calmed, enabled
    // zooscopy: omen, max_omen, divine_rate, just_revealed, just_obscured, enabled
    // zootherapy: relief, max_relief, heal_rate, just_healed, just_relapsed, enabled
    // zoochemistry: compound, max_compound, brew_rate, just_brewed, just_neutralized, enabled
    // zoochory: seed, max_seed, disperse_rate, just_dispersed, just_contained, enabled
    // zoocide: toll, max_toll, kill_rate, just_exterminated, just_replenished, enabled
    // zooecium: shell, max_shell, encase_rate, just_encased, just_exposed, enabled
    // zooflagellate: thrust, max_thrust, propel_rate, just_propelled, just_stalled, enabled
    // zoogenic: vitality, max_vitality, generate_rate, just_generated, just_suppressed, enabled
    // zoohygric: moisture, max_moisture, hydrate_rate, just_hydrated, just_desiccated, enabled
    // zooculture: breed, max_breed, cultivate_rate, just_cultivated, just_abandoned, enabled
    // zoolite: fossil, max_fossil, petrify_rate, just_petrified, just_eroded, enabled
    // zoonomy: norm, max_norm, regulate_rate, just_regulated, just_disrupted, enabled
    // zoopery: trial, max_trial, probe_rate, just_probed, just_halted, enabled
    // zoophysics: force, max_force, channel_rate, just_channeled, just_scattered, enabled
    // zoosemiotics: signal, max_signal, transmit_rate, just_transmitted, just_silenced, enabled
    // zootrophy: nutrient, max_nutrient, feed_rate, just_fed, just_starved, enabled
    // zooarchaeology: artifact, max_artifact, excavate_rate, just_excavated, just_buried, enabled
    // zoobiotic: parasite, max_parasite, infest_rate, just_infested, just_cleansed, enabled
    // zoodynamics: momentum, max_momentum, drive_rate, just_driven, just_braked, enabled
    // zoognosis: insight, max_insight, diagnose_rate, just_diagnosed, just_misled, enabled
    // zookinesis: motion, max_motion, mobilize_rate, just_mobilized, just_immobilized, enabled
    // zoomorphosis: form, max_form, morph_rate, just_morphed, just_reverted, enabled
    // zoopsia: vision, max_vision, perceive_rate, just_perceived, just_blinded, enabled
    // zoocoenosis: colony, max_colony, assemble_rate, just_assembled, just_dissolved, enabled
    // zoodendrium: branch, max_branch, extend_rate, just_extended, just_pruned, enabled
    // zoomancy: portent, max_portent, augur_rate, just_augured, just_dispelled, enabled
    // zooparasite: host, max_host, latch_rate, just_latched, just_expelled, enabled
    // zoopathia: malady, max_malady, ail_rate, just_ailed, just_recovered, enabled
    // zooplasty: graft, max_graft, fuse_rate, just_fused, just_rejected, enabled
    // zoosporangia: spore, max_spore, sporulate_rate, just_sporulated, just_depleted, enabled
    // zooblast: blast, max_blast, proliferate_rate, just_proliferated, just_lysed, enabled
    // zoocarp: propagule, max_propagule, release_rate, just_released, just_aborted, enabled
    // zoodeme: population, max_population, aggregate_rate, just_aggregated, just_dispersed, enabled
    // zoomass: biomass, max_biomass, accumulate_rate, just_accumulated, just_decomposed, enabled
    // zoophagy: prey, max_prey, hunt_rate, just_hunted, just_fled, enabled
    // zoosterol: sterol, max_sterol, synthesize_rate, just_synthesized, just_catabolized, enabled
    // zootoxin: toxin, max_toxin, secrete_rate, just_secreted, just_neutralized, enabled
    // zoochromy: pigment, max_pigment, tint_rate, just_tinted, just_faded, enabled
    // zooecology: habitat, max_habitat, settle_rate, just_settled, just_displaced, enabled
    // zoomorphology: form, max_form, develop_rate, just_developed, just_regressed, enabled
    // zoopsychology: instinct, max_instinct, imprint_rate, just_imprinted, just_unlearned, enabled
    // zoosociology: bond, max_bond, affiliate_rate, just_affiliated, just_severed, enabled
    // zootaxy: rank, max_rank, promote_rate, just_promoted, just_demoted, enabled
    // zootheism: devotion, max_devotion, consecrate_rate, just_consecrated, just_desecrated, enabled
    // zooanthropy: delusion, max_delusion, impart_rate, just_imparted, just_dispelled, enabled
    // zoogonium: spore, max_spore, produce_rate, just_produced, just_consumed, enabled
    // zoolemma: integrity, max_integrity, repair_rate, just_repaired, just_breached, enabled
    // zoopharmacology: potency, max_potency, dose_rate, just_dosed, just_purged, enabled
    // zooplasma: density, max_density, condense_rate, just_condensed, just_dispersed, enabled
    // zoospermia: motility, max_motility, energize_rate, just_energized, just_depleted, enabled
    // zoosymbiont: harmony, max_harmony, mutualize_rate, just_mutualized, just_expelled, enabled
    // entity name → (current, max)
    pub(crate) static SHIELD_SNAPSHOT: RefCell<HashMap<String, (f32, f32)>> =
        RefCell::new(HashMap::new());

    // entity name → (level, current_xp, progress, is_max)

    // entity name → (current, max, prestige, is_max, progress_fraction)

    // entity name → (remaining, progress, is_ready)

    // entity name → (elapsed, duration, fraction, is_finished, just_finished)
    pub(crate) static TIMER_SNAPSHOT: RefCell<HashMap<String, (f32, f32, f32, bool, bool)>> =
        RefCell::new(HashMap::new());

    // entity name → (current, max_capacity, reserve, reserve_max, just_emptied, just_reloaded, enabled)

    // entity name → (rate, delay_after_damage, delay_timer, enabled)

    // entity name → (fuel, max_fuel, low_threshold, just_emptied, is_low, enabled)

    // entity name → (current, max_charge, is_charging, is_fully_charged, enabled)

    // entity name → (flat_reduction, percent_reduction, durability, max_durability, enabled)

    // entity name → (impulse, max_jumps, jumps_remaining, wants_jump, enabled)

    // entity name → (speed_multiplier, is_sprinting, is_exhausted, just_started, just_stopped, enabled)

    // entity name → (speed, duration, cooldown, cooldown_timer, max_charges, charges, is_active, is_invincible, can_dash, enabled)

    // entity name → (speed, angular_speed, stopping_distance, state_u8, enabled)
    // state: 0=Idle, 1=Moving, 2=Arrived, 3=NoPath
    pub(crate) static NAV_SNAPSHOT: RefCell<HashMap<String, (f32, f32, f32, u8, bool)>> =
        RefCell::new(HashMap::new());

    // entity name → (force, vertical_boost, hits_remaining, blocks_new, enabled)

    // entity name → (speed, gravity_scale, piercing, range, distance_traveled)

    // entity name → (state_u8, anchor_x, anchor_y, anchor_z, max_range, hook_speed, pull_force, rope_length, enabled)

    // entity name → (range, prompt, trigger_u8, hold_duration, enabled)

    // entity name → (trauma, amplitude, decay_rate, frequency)

    // entity name → (step_interval, distance_accumulated, volume, audio_prefix, surface_u8, min_speed, enabled)

    // entity name → (vx, vy, vz, turbulence, turbulence_frequency, radius)

    // entity name → (current_index, line_count, looping, enabled, is_finished, current_speaker, current_text)

    // entity name → (progress, edge_width, edge_r, edge_g, edge_b, edge_a, noise_scale, enabled)

    // entity name → (color_r, color_g, color_b, color_a, intensity, contributes_to_bloom, enabled)

    // entity name → (cell_x, cell_y, cell_z, off_x, off_y, off_z, enabled)

    // entity name → (style_u8, color_r, color_g, color_b, color_a, size, thickness, gap, spread, max_spread, spread_decay, enabled)

    // entity name → (intensity, threshold, radius, softness, enabled)
    pub(crate) static BLOOM_SNAPSHOT: RefCell<HashMap<String, (f32, f32, f32, f32, bool)>> =
        RefCell::new(HashMap::new());

    // entity name → (color_r, color_g, color_b, intensity, mode_u32, fade_rate, pulse_speed, pulse_phase, peak_intensity, enabled)

    // entity name → (intensity, enabled)

    // entity name → (lut_path, exposure, contrast, saturation, hue_shift, brightness, enabled)

    // entity name → (radius, bias, intensity, sample_count, enabled)
    pub(crate) static AMBIENT_OCCLUSION_SNAPSHOT: RefCell<HashMap<String, (f32, f32, f32, u32, bool)>> =
        RefCell::new(HashMap::new());

    // entity name → (focal_distance, focal_range, max_blur, bokeh_scale, enabled)

    // entity name → (shutter_angle, sample_count, enabled)

    // entity name → (mode_u32, exposure, enabled)
    // ToneMappingMode: None=0, Reinhard=1, ReinhardLuminance=2, Aces=3, Filmic=4
    pub(crate) static TONE_MAP_SNAPSHOT: RefCell<HashMap<String, (u32, f32, bool)>> =
        RefCell::new(HashMap::new());

    // entity name → (fraction, pool, max_pool, absorbed_total, just_depleted, enabled)

    // entity name → (intensity, smoothness, r, g, b, enabled)
    // entity name → (r, g, b, a, density, start_distance, end_distance, mode_u32, enabled)
    // FogMode: Linear=0, Exponential=1, ExponentialSquared=2
    // entity name → (target_name, rest_length, stiffness, damping, break_ext, enabled)
    // break_ext = -1.0 means no break limit (Option::None)
    // entity name → (target_type, duration, easing_u32, repeat_u32, elapsed, finished, reversed)
    // TweenTarget: Translation=0, Rotation=1, Scale=2
    // EasingFn: Linear=0, EaseInQuad=1, EaseOutQuad=2, EaseInOutQuad=3
    // RepeatMode: Once=0, Loop=1, PingPong=2
    pub(crate) static TWEEN_SNAPSHOT: RefCell<HashMap<String, (u32, f32, u32, u32, f32, bool, bool)>> =
        RefCell::new(HashMap::new());
    // entity name → (fluid_density, volume, linear_drag, angular_drag, surface_y)
    // entity name → (target_name, offset_x, offset_y, offset_z, speed)
    pub(crate) static FOLLOW_SNAPSHOT: RefCell<HashMap<String, (String, f32, f32, f32, f32)>> =
        RefCell::new(HashMap::new());
    // entity name → (target_name, up_x, up_y, up_z)
    pub(crate) static LOOK_AT_SNAPSHOT: RefCell<HashMap<String, (String, f32, f32, f32)>> =
        RefCell::new(HashMap::new());
    // entity name → mode (0 = Full, 1 = Vertical)
    // entity name → (r, g, b, a, width, mode, visible)
    // OutlineMode: Outer=0, Inner=1, Center=2
    // entity name → z-index (i32; 0 = default draw order)
    // entity name → layer bitmask (u32)
    // entity name → (preset_u32, norm_x, norm_y, offset_x, offset_y)
    // AnchorPreset: Center=0, TopLeft=1, TopCenter=2, TopRight=3,
    //   MiddleLeft=4, MiddleRight=5, BottomLeft=6, BottomCenter=7, BottomRight=8, Custom=9
    // entity name → (layer_mask, enabled)
    // entity name → (amount, type_u32, custom_id, multiplier, piercing)
    // DamageType: Physical=0, Fire=1, Ice=2, Lightning=3, Poison=4, Custom=5 (custom_id relevant)
    // entity name → (tag, team_u32, enabled); team=u32::MAX means None/shared
    // entity name → (id, kind_u32, custom_id, value, duration, ticks_every_frame, enabled)
    // EffectKind: StatMultiplier=0, DamageOverTime=1, Immobilize=2, Silence=3, Custom=4
    // entity name → (ability_name, cooldown, cooldown_remaining, max_charges, charges, charge_regen_time, charge_regen_accumulated, enabled)
    // entity name → (alert_duration, timer, detection_radius, just_triggered, just_calmed, enabled)
    // entity name → (duration, timer, power_multiplier, just_amplified, just_faded, enabled)
    // entity name → (capacity, current, regen_rate, regen_delay, regen_timer, just_broken, just_restored, enabled)
    // entity name → (priority, broadcast_radius, duration, timer, lit, just_lit, just_extinguished, enabled)
    // entity name → (duration, timer, reduction_fraction, just_broken, just_recovered, enabled)
    // entity name → (duration, timer, allows_rotation, allows_attack, just_rooted, just_freed, enabled)
    // entity name → (reduction, duration, timer, just_slowed, just_recovered, enabled)
    // entity name → (severity_u32, timer, just_stunned, just_recovered, enabled)
    // StunSeverity: Light=0, Heavy=1, Knockdown=2
    // entity name → (burn_rate, stacks, max_stacks, remaining, duration, intensity, just_ignited, just_extinguished, ignitable, enabled)
    // entity name → (stacks, max_stacks, damage_per_stack_per_tick, tick_interval, tick_timer, duration, duration_timer, heal_reduction, just_applied, just_cleared, enabled)
    // entity name → (stacks, max_stacks, damage_per_stack_per_tick, base_tick_interval, min_tick_interval, tick_timer, duration, duration_timer, virulent, just_poisoned, just_cured, enabled)
    // entity name → (state_u32, cold_buildup, chill_threshold, freeze_threshold, cold_decay_rate, chill_slow, frozen_duration, frozen_timer, just_frozen, just_thawed, immune, enabled)
    // FreezeState: Normal=0, Chilled=1, Frozen=2
    // entity name → (duration, timer, range_limit, aim_deviation_rad, just_blinded, just_unblinded, enabled)
    // entity name → (duration, timer, just_charmed, just_uncharmed, enabled)
    // entity name → (duration, timer, chance, just_confused, just_unconfused, enabled)
    // entity name → (duration, timer, speed_fraction, prevents_jump, just_crippled, just_recovered, enabled)
    // entity name → (duration, timer, slow_fraction, aim_deviation_rad, just_dazed, just_undazed, enabled)
    // entity name → (duration, timer, just_disarmed, just_rearmed, enabled)
    // entity name → (duration, timer, aim_deviation_rad, ability_suppress_chance, just_concussed, just_cleared, enabled)
    // entity name → (stacks, max_stacks, decay_rate, armor_reduction_per_stack, just_corroded, just_cleared, enabled)
    // entity name → (kind_u32, strength, duration, timer, just_cursed, just_lifted, enabled)
    // CurseKind: DamageDown=0, SpeedDown=1, ArmorDown=2, DamageTakenUp=3, Custom=4
    // entity name → (radius, pulse_interval, pulse_timer, buildup_per_pulse, just_pulsed, enabled)
    // entity name → (active, countdown, max_countdown, just_doomed, just_expired, enabled)
    // entity name → (duration, timer, damage_fraction, flee_chance, just_demoralized, just_recovered, enabled)
    // entity name → (phase_u32, dx, dy, dz, speed, duration, timer, invincible, cooldown, wants_dodge, allow_airborne, chain_count, max_chain, enabled)
    // DodgePhase: Idle=0, Rolling=1, Cooldown=2
    // entity name → (rate, duration, timer, just_drained, just_expired, enabled)
    // entity name → (duration, timer, potency_multiplier, just_empowered, just_faded, enabled)
    // entity name → (duration, timer, regen_fraction, max_pool_fraction, just_enervated, just_restored, enabled)
    // entity name → (duration, timer, just_entangled, just_unentangled, enabled)
    // entity name → (duration, timer, damage_multiplier, just_exposed, just_recovered, enabled)
    // entity name → (level, recovery_rate, threshold, penalty_speed, penalty_regen, just_exhausted, just_recovered, enabled)
    // entity name → (state_u32, duration, timer, flee_speed_multiplier, just_feared, just_calmed, enabled)
    // FearState: Calm=0, Frightened=1, Fleeing=2
    // entity name → (duration, timer, damage_amplification, move_speed_penalty, just_fractured, just_healed, enabled)
    // entity name → (duration, timer, cold_damage_per_second, action_speed_fraction, just_frostbitten, just_thawed, enabled)
    // entity name → (fury_factor, max_speed_bonus, just_peaked, enabled)
    // entity name → (duration, timer, speed_multiplier, just_galvanized, just_worn_off, enabled)
    // entity name → (effective_multiplier, stack_count, max_stacks, enabled)
    // entity name → (duration, timer, stray_chance, damage_multiplier, just_entered, just_exited, enabled)
    // entity name → (duration, timer, detection_range_fraction, just_hazed, just_cleared, enabled)
    // entity name → (temperature, resting_temp, heat_threshold, cold_threshold, decay_rate, resistance, state_u32, enabled)
    // ThermalState: Normal=0, Overheated=1, Frozen=2
    // entity name → (stacks, max_stacks, duration, timer, reduction_per_stack, just_applied, just_expired, enabled)
    // entity name → (duration, timer, speed_fraction, prevents_dash, just_hobbled, just_recovered, enabled)
    // entity name → (stacks, threshold, decay_rate, just_ignited, just_extinguished, enabled)
    // entity name → (charged, bonus_damage, just_charged, just_consumed, enabled)
    // entity name → (damage_type_mask, effect_type_mask, enabled)
    // entity name → (force, just_impacted, impact_count, normal_x, normal_y, normal_z, enabled)
    // entity name → (duration, timer, radius, damage_reduction, just_activated, just_deactivated, enabled)
    // entity name → (threshold, resistance, just_interrupted, interrupt_count, enabled)
    // entity name → (stacks, timer, flash_interval, flash_visible, just_became_invincible, just_lost_invincibility, enabled)
    // entity name → (duration, timer, buff_reduction, debuff_reduction, just_began, just_ended, enabled)
    // entity name → (duration, timer, aim_penalty_rad, damage_fraction, just_jeered, just_rallied, enabled)
    // entity name → (state_u32, thrust_x, thrust_y, thrust_z, thrust_force, fuel, max_fuel, fuel_drain_rate, fuel_regen_rate, wants_thrust, regen_in_air, enabled)
    // JetpackState: Idle=0, Thrusting=1, Depleted=2
    // entity name → (duration, timer, chain_chance, chain_fraction, just_jolted, just_expired, enabled)
    // entity name → (accumulated, threshold, decay_rate, just_destabilized, enabled)
    // entity name → (charges, max_charges, just_juked, enabled)
    // entity name → (duration, timer, speed_fraction, just_kneeled, just_risen, enabled)
    // entity name → (duration, timer, heal_rate, interruption_threshold, just_began, just_completed, just_interrupted, enabled)
    // entity name → (stacks, max_stacks, damage_per_stack_per_second, duration, timer, just_lacerated, just_closed, enabled)
    // entity name → (current_load, max_load, speed_penalty, enabled)
    // entity name → (intensity, decay_rate, damage_penalty, speed_penalty, just_lamented, just_recovered, enabled)
    // entity name → (duration, timer, base_damage, speed_scale, speed_threshold, just_struck, just_ended, enabled)
    // entity name → (lapsing, interval_timer, duration_timer, interval, lapse_duration, just_lapsed, just_focused, enabled)
    // entity name → (pull_force, damage, duration, timer, just_connected, just_released, enabled)
    // entity name → (active, timer, damage_per_second, just_latched, just_released, enabled)
    // entity name → (phase_u32, hang_x, hang_y, hang_z, climb_duration, climb_timer, detection_range, can_grab, enabled)
    // LedgePhase: None=0, Hanging=1, ClimbingUp=2, Dropping=3
    // entity name → (fraction, flat, last_leeched, total_leeched, just_leeched, enabled)
    // entity name → (phase_u32, dir_x, dir_y, dir_z, target_x, target_y, target_z, speed, range, traveled, recovery_time, recovery_timer, cooldown, cooldown_timer, ground_only, just_lunged, hit_registered, enabled)
    // LungePhase: Idle=0, Thrusting=1, Recovery=2, Cooldown=3
    // entity name → (state_u32, pos_x, pos_y, pos_z, radius, strength, duration, timer, just_activated, just_expired, enabled)
    // LureState: Inactive=0, Active=1, Expired=2
    // entity name → (detection_range_fraction, ambush_multiplier, lurking, just_lurked, just_struck, enabled)
    // entity name → (mode_u32, radius, strength, falloff, affects_projectiles, affects_entities, enabled)
    // MagnetMode: Attract=0, Repel=1
    // entity name → (stacks, max_stacks, speed_fraction_per_stack, bleed_per_stack_per_second, just_maimed, just_healed, enabled)
    // entity name → (stacks, max_stacks, damage_amplify_per_stack, decay_interval, decay_timer, just_stacked, just_cleared, enabled)
    // entity name → (mark_count, total_damage_bonus, just_marked, just_unmarked, enabled)
    // entity name → (phase_u32, dir_x, dir_y, dir_z, reach, arc_angle, windup_time, active_time, recovery_time, timer, hit_count, max_hits, combo_step, combo_buffered, can_cancel_recovery, enabled)
    // MeleePhase: Idle=0, Windup=1, Active=2, Recovery=3
    // entity name → (mend_pool, rate, just_depleted, enabled)
    // entity name → (can_merge, merge_weight, max_weight, just_merged, enabled)
    // entity name → (path, submesh_index, cast_shadow, receive_shadow)
    // entity name → (icon, cr, cg, cb, ca, size, category, rotate_with_entity, clamp_to_edge, enabled)
    // entity name → (duration, timer, misdirect_chance, just_created, just_faded, enabled)
    // entity name → (cur_x, cur_y, cur_z, damping, max_speed, enabled)
    // entity name → (morale, decay_rate, damage_bonus, speed_bonus, just_peaked, just_broke, enabled)
    // entity name → (form, target_form, morph_time, morph_timer, is_morphing, just_started, just_finished, enabled)
    // entity name → (rider_count, max_riders, speed_scale, forced_dismount_damage, enabled)
    // forced_dismount_damage = -1.0 if None
    // entity name → (duration, timer, sound_radius_fraction, just_muffled, just_unmuffled, enabled)
    // entity name → (id_str, authority_kind[0=Server,1=Client,2=Local], peer_id_str)
    pub(crate) static NETWORK_ID_SNAPSHOT: RefCell<HashMap<String, (String, u32, String)>> =
        RefCell::new(HashMap::new());
    // (is_server, is_connected, my_peer_id, peer_count)
    pub(crate) static NETWORK_STATE_SNAPSHOT: RefCell<(bool, bool, u64, u32)> =
        const { RefCell::new((false, false, 0, 0)) };
    // entity name → (duration, timer, dodge_chance, speed_bonus_fraction, just_quickened, just_faded, enabled)
    // entity name → (state_u32, suspicion, decay_rate, alert_threshold, alarm_threshold, last_x, last_y, last_z, investigate_timer, max_investigate_time, has_last_known, enabled)
    // entity name → (satiety, decay_rate, regen_scale, just_starved, enabled)
    // entity name → (charge_time, charge_timer, radius, damage, just_primed, just_discharged, enabled)
    // entity name → (role_u32, state_u32, display_name, template_id_or_empty, faction_id, alert, alert_decay, enabled)
    // entity name → (duration, timer, blocks_buffs, blocks_debuffs, just_activated, just_expired, enabled)
    // entity name → (duration, timer, damage_fraction, just_numbed, just_worn_off, enabled)
    // entity name → (shape_kind[0=Circle,1=Box,2=Capsule], p1, p2, p3, dynamic, carve_depth, bounding_radius, enabled)
    // entity name → (stacks, max_stacks, dmg_mult_per_stack, just_stacked, just_consumed, enabled)
    // entity name → (radius, speed, angle, ax, ay, az, altitude, enabled)
    // entity name → (duration, timer, just_began, just_endured, just_failed, enabled)
    // entity name → (axis_kind[0=Translation,1=Rotation], dx, dy, dz, amplitude, frequency, phase, phase_offset, scalar_offset, enabled)
    // entity name → (combat_time, max_bonus_time, defense_bonus, in_combat, just_peaked, enabled)
    // entity name → (current, max_pool, decay_rate, just_gained, just_depleted, enabled)
    // entity name → (state[0=Normal,1=Warning,2=Overheated,3=Cooling], heat, max_heat, warn_threshold, cool_threshold, cool_rate, forced_cool_rate, just_overheated, just_cooled, enabled)
    // entity name → (duration, timer, cost_multiplier, just_overloaded, just_recovered, enabled)
    // entity name → (duration, timer, armor_penetration, just_overpowered, just_faded, enabled)
    // entity name → (current, max_overshield, decay_rate, just_granted, just_depleted, enabled)
    // entity name → (state_kind, startup_dur, active_dur, recovery_dur, timer, parry_count, just_opened, just_succeeded, just_missed, just_finished, enabled)
    // entity name → (patience_level, max_patience, patience_bonus, just_primed, just_spent, enabled)
    // entity name → (duration, timer, armor_bonus, just_petrified, just_unpetrified, enabled)
    // entity name → (is_phased, duration, timer, cooldown, cooldown_timer, just_phased, just_unphased, enabled)
    // entity name → (max_pierce, pierce_chance, pierced_this_attack, just_pierced, enabled)
    // entity name → (active, timer, duration, knockback_immune, just_pinned, just_freed, enabled)
    // entity name → (duration, timer, avoidance_chance, just_began, just_ended, enabled)
    // entity name → (active, timer, just_began, just_ended, enabled)
    // entity name → (hp_threshold, crit_bonus, pluck_active, just_triggered, just_recovered, enabled)
    // entity name → (current, max, regen_rate, broken, just_broken, just_restored, enabled)
    // entity name → (duration, timer, damage, knockdown_duration, min_range, max_range, just_leaped, just_landed, enabled)
    // entity name → (is_prone, stand_up_duration, stand_up_timer, movement_penalty, attack_penalty, just_fell_prone, just_stood_up, enabled)
    // entity name → (duration, timer, guard_radius, redirect_fraction, just_began, just_ended, enabled)
    // entity name → (hp_threshold, damage_bonus, prideful, just_humbled, just_restored, enabled)
    // entity name → (duration, timer, aggro_multiplier, radius, just_provoked, just_expired, enabled)
    // entity name → (duration, timer, speed_bonus_fraction, ambush_damage_multiplier, ambush_consumed, just_prowling, just_faded, enabled)
    // entity name → (mode_kind, is_active, radius, max_radius, interval, timer, falloff, pulse_count, just_pulsed, enabled)
    // entity name → (state, xp_reward, enabled)
    // entity name → (range, scan_interval, scan_timer, enabled)
    // entity name → (phase, rage, max_rage, rage_per_damage, activation_threshold, damage_multiplier, defense_multiplier, just_entered_rage, just_left_rage, enabled)
    // entity name → (duration, timer, aura_radius, speed_bonus_fraction, damage_bonus_fraction, just_rallied, just_ended, enabled)
    // entity name → (stacks, max_stacks, damage_per_stack, speed_per_stack, decay_interval, decay_timer, just_stacked, just_ended, enabled)
    // entity name → (active, timer, damage_bonus, attack_speed_bonus, just_triggered, just_expired, enabled)
    // entity name → (duration, timer, leech_fraction, just_reaving, just_faded, enabled)
    // entity name → (rebound_coefficient, min_speed, last_rebound_speed, just_rebounded, enabled)
    // entity name → (current, max, rate, just_recharged, just_depleted, enabled)
    // entity name → (duration, timer, damage_bonus, defense_penalty, just_entered, just_exited, enabled)
    // entity name → (is_alone, damage_bonus, defense_bonus, just_became_alone, just_joined_group, enabled)
    // entity name → (kick_force, angular_kick, recovery_speed, yaw_fraction, max_position_offset, max_angular_offset, enabled)
    // entity name → (is_active, damage_multiplier, window_duration, window_timer, just_activated, just_reflected, just_closed, enabled)
    // entity name → (timer, just_triggered, just_evaded, just_missed, enabled)
    // entity name → (duration, timer, push_force, radius, just_activated, just_deactivated, enabled)
    // entity name → (active, timer, regen_multiplier, just_began, just_ended, enabled)
    // entity name → (state, delay, delay_timer, respawn_count, enabled)
    // entity name → (multiplier, max_charges, charges, just_charged, just_consumed, enabled)
    // entity name → (duration, timer, revenge_multiplier, trigger_fraction, triggered, just_triggered, just_ended, enabled)
    // entity name → (duration, timer, radius, just_activated, just_expired, enabled)
    // entity name → (state, down_duration, down_timer, revive_duration, revive_progress, revives_remaining, just_downed, just_revived, just_died, enabled)
    // entity name → (max_bounces, bounces_remaining, energy_retention, min_dot, just_bounced, enabled)
    // entity name → (min_range, peak_range, damage_bonus, point_blank_penalty, enabled)
    // entity name → (active, decay_rate, total_decayed, decay_cap, just_began, just_capped, enabled)
    // entity name → (duration, timer, flee_speed_multiplier, just_routed, just_recovered, enabled)
    // entity name → (stacks, max_stacks, damage_per_stack, just_maxed, enabled)
}

/// Full transform returned to scripts: position + rotation quaternion + scale.
#[derive(Serialize)]
struct TransformJson {
    x: f32,
    y: f32,
    z: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    rw: f32,
    sx: f32,
    sy: f32,
    sz: f32,
}

#[derive(Serialize)]
struct RaycastHitJson {
    entity_name: Option<String>,
    point: [f32; 3],
    normal: [f32; 3],
    distance: f32,
}

#[op2(fast)]
pub fn bsengine_log(#[string] msg: String) {
    tracing::info!("[script] {}", msg);
}

#[op2]
#[string]
pub fn bsengine_version() -> String {
    "0.1.0".to_string()
}

#[op2]
#[serde]
pub fn bsengine_get_transform(#[string] name: String) -> Option<TransformJson> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(pos, rot, scale)| TransformJson {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                rx: rot.x,
                ry: rot.y,
                rz: rot.z,
                rw: rot.w,
                sx: scale.x,
                sy: scale.y,
                sz: scale.z,
            })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_forward_vector(#[string] name: String) -> Option<Vec<f32>> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|(_, rot, _)| {
            let v = rot.mul_vec3(Vec3::NEG_Z);
            vec![v.x, v.y, v.z]
        })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_right_vector(#[string] name: String) -> Option<Vec<f32>> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|(_, rot, _)| {
            let v = rot.mul_vec3(Vec3::X);
            vec![v.x, v.y, v.z]
        })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_up_vector(#[string] name: String) -> Option<Vec<f32>> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|(_, rot, _)| {
            let v = rot.mul_vec3(Vec3::Y);
            vec![v.x, v.y, v.z]
        })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_world_transform(#[string] name: String) -> Option<TransformJson> {
    WORLD_TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(pos, rot, scale)| TransformJson {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                rx: rot.x,
                ry: rot.y,
                rz: rot.z,
                rw: rot.w,
                sx: scale.x,
                sy: scale.y,
                sz: scale.z,
            })
    })
}

#[op2(fast)]
pub fn bsengine_distance_to(#[string] name_a: String, #[string] name_b: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        let pos_a = snap.get(&name_a).map(|(p, _, _)| *p);
        let pos_b = snap.get(&name_b).map(|(p, _, _)| *p);
        match (pos_a, pos_b) {
            (Some(a), Some(b)) => a.distance(b),
            _ => -1.0,
        }
    })
}

#[op2(fast)]
pub fn bsengine_distance_to_point(#[string] name: String, x: f32, y: f32, z: f32) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(pos, _, _)| pos.distance(Vec3::new(x, y, z)))
            .unwrap_or(-1.0)
    })
}

#[op2(fast)]
pub fn bsengine_set_transform(#[string] name: String, x: f32, y: f32, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetTransform { name, x, y, z });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation(#[string] name: String, rx: f32, ry: f32, rz: f32, rw: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetRotation {
            name,
            rx,
            ry,
            rz,
            rw,
        });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler(
    #[string] name: String,
    pitch_deg: f32,
    yaw_deg: f32,
    roll_deg: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetRotationEuler {
            name,
            pitch_deg,
            yaw_deg,
            roll_deg,
        });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale(#[string] name: String, sx: f32, sy: f32, sz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetScale { name, sx, sy, sz });
    });
}

#[op2(fast)]
pub fn bsengine_add_position(#[string] name: String, dx: f32, dy: f32, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPosition { name, dx, dy, dz });
    });
}

#[op2(fast)]
pub fn bsengine_rotate_by(#[string] name: String, rx: f32, ry: f32, rz: f32, rw: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::RotateBy {
            name,
            rx,
            ry,
            rz,
            rw,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_local(#[string] name: String, dx: f32, dy: f32, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionLocal { name, dx, dy, dz });
    });
}

#[op2(fast)]
pub fn bsengine_set_position_x(#[string] name: String, x: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetPositionX { name, x });
    });
}

#[op2(fast)]
pub fn bsengine_set_position_y(#[string] name: String, y: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetPositionY { name, y });
    });
}

#[op2(fast)]
pub fn bsengine_set_position_z(#[string] name: String, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetPositionZ { name, z });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_x(#[string] name: String, dx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionX { name, dx });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_y(#[string] name: String, dy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionY { name, dy });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_z(#[string] name: String, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionZ { name, dz });
    });
}

#[op2(fast)]
pub fn bsengine_rotate_around_axis(
    #[string] name: String,
    ax: f32,
    ay: f32,
    az: f32,
    angle_deg: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::RotateAroundAxis {
            name,
            ax,
            ay,
            az,
            angle_deg,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler(#[string] name: String, pitch: f32, yaw: f32, roll: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddRotationEuler {
            name,
            pitch,
            yaw,
            roll,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler_x(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddRotationEulerX { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler_y(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddRotationEulerY { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler_z(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddRotationEulerZ { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale_x(#[string] name: String, x: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetScaleX { name, x });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale_y(#[string] name: String, y: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetScaleY { name, y });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale_z(#[string] name: String, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetScaleZ { name, z });
    });
}

#[op2(fast)]
pub fn bsengine_add_scale_x(#[string] name: String, dx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddScaleX { name, dx });
    });
}

#[op2(fast)]
pub fn bsengine_add_scale_y(#[string] name: String, dy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddScaleY { name, dy });
    });
}

#[op2(fast)]
pub fn bsengine_add_scale_z(#[string] name: String, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddScaleZ { name, dz });
    });
}

#[op2(fast)]
pub fn bsengine_get_position_x(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.0.x))
}

#[op2(fast)]
pub fn bsengine_get_position_y(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.0.y))
}

#[op2(fast)]
pub fn bsengine_get_position_z(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.0.z))
}

#[op2(fast)]
pub fn bsengine_get_scale_x(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.2.x))
}

#[op2(fast)]
pub fn bsengine_get_scale_y(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.2.y))
}

#[op2(fast)]
pub fn bsengine_get_scale_z(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.2.z))
}

#[op2(fast)]
pub fn bsengine_get_rotation_euler_x(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map_or(f32::NAN, |t| {
            let (x, _, _) = t.1.to_euler(glam::EulerRot::XYZ);
            x.to_degrees()
        })
    })
}

#[op2(fast)]
pub fn bsengine_get_rotation_euler_y(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map_or(f32::NAN, |t| {
            let (_, y, _) = t.1.to_euler(glam::EulerRot::XYZ);
            y.to_degrees()
        })
    })
}

#[op2(fast)]
pub fn bsengine_get_rotation_euler_z(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map_or(f32::NAN, |t| {
            let (_, _, z) = t.1.to_euler(glam::EulerRot::XYZ);
            z.to_degrees()
        })
    })
}

#[op2(fast)]
pub fn bsengine_add_scale(#[string] name: String, sx: f32, sy: f32, sz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddScale { name, sx, sy, sz });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler_x(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRotationEulerX { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler_y(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRotationEulerY { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler_z(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRotationEulerZ { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_multiply_scale(#[string] name: String, sx: f32, sy: f32, sz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::MultiplyScale { name, sx, sy, sz });
    });
}

#[op2(fast)]
pub fn bsengine_is_key_pressed(#[string] key: String) -> bool {
    KEY_SNAPSHOT.with(|k| k.borrow().contains(&key))
}

#[op2(fast)]
pub fn bsengine_is_key_down(#[string] key: String) -> bool {
    KEY_JUST_PRESSED_SNAPSHOT.with(|k| k.borrow().contains(&key))
}

#[op2(fast)]
pub fn bsengine_is_key_up(#[string] key: String) -> bool {
    KEY_JUST_RELEASED_SNAPSHOT.with(|k| k.borrow().contains(&key))
}

#[op2]
#[string]
pub fn bsengine_get_entity_names() -> String {
    ENTITY_NAMES_SNAPSHOT
        .with(|s| serde_json::to_string(&*s.borrow()).unwrap_or_else(|_| "[]".to_string()))
}

#[op2(fast)]
pub fn bsengine_entity_exists(#[string] name: String) -> bool {
    ENTITY_NAMES_SNAPSHOT.with(|s| s.borrow().contains(&name))
}

#[op2(fast)]
pub fn bsengine_get_entity_count() -> u32 {
    ENTITY_NAMES_SNAPSHOT.with(|s| s.borrow().len() as u32)
}

#[op2]
#[string]
pub fn bsengine_get_entities_in_radius(x: f32, y: f32, z: f32, radius: f32) -> String {
    let center = Vec3::new(x, y, z);
    let r2 = radius * radius;
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        let names: Vec<&str> = snap
            .iter()
            .filter(|(_, (pos, _, _))| pos.distance_squared(center) <= r2)
            .map(|(name, _)| name.as_str())
            .collect();
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    })
}

#[op2]
#[string]
pub fn bsengine_get_closest_entity(x: f32, y: f32, z: f32) -> String {
    let center = Vec3::new(x, y, z);
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        snap.iter()
            .min_by(|(_, (pa, _, _)), (_, (pb, _, _))| {
                pa.distance_squared(center)
                    .partial_cmp(&pb.distance_squared(center))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone())
            .unwrap_or_default()
    })
}

#[op2(fast)]
pub fn bsengine_set_emissive(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetEmissive { name, r, g, b });
    });
}

#[op2(fast)]
pub fn bsengine_set_color(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetColor { name, r, g, b });
    });
}

#[op2]
pub fn bsengine_spawn(#[serde] params: SpawnParams) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::Spawn(params)));
}

#[op2(fast)]
pub fn bsengine_destroy(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::Destroy { name }));
}

#[op2(fast)]
pub fn bsengine_set_visible(#[string] name: String, visible: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVisible { name, visible });
    });
}

#[op2(fast)]
pub fn bsengine_get_visible(#[string] name: String) -> bool {
    VISIBLE_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(true))
}

#[op2]
#[serde]
pub fn bsengine_get_material_color(#[string] name: String) -> Option<Vec<f32>> {
    MATERIAL_COLOR_SNAPSHOT.with(|s| s.borrow().get(&name).map(|c| c.to_vec()))
}

#[op2]
#[serde]
pub fn bsengine_get_material_emissive(#[string] name: String) -> Option<Vec<f32>> {
    MATERIAL_EMISSIVE_SNAPSHOT.with(|s| s.borrow().get(&name).map(|c| c.to_vec()))
}

#[op2(fast)]
pub fn bsengine_set_metallic(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetMetallic { name, value });
    });
}

#[op2(fast)]
pub fn bsengine_get_metallic(#[string] name: String) -> f32 {
    MATERIAL_METALLIC_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(f32::NAN))
}

#[op2(fast)]
pub fn bsengine_set_roughness(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRoughness { name, value });
    });
}

#[op2(fast)]
pub fn bsengine_get_roughness(#[string] name: String) -> f32 {
    MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(f32::NAN))
}

#[op2(fast)]
pub fn bsengine_set_point_light_color(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetPointLightColor { name, r, g, b })
    });
}

#[op2(fast)]
pub fn bsengine_set_point_light_intensity(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetPointLightIntensity { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_set_point_light_range(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetPointLightRange { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_set_spot_light_color(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSpotLightColor { name, r, g, b })
    });
}

#[op2(fast)]
pub fn bsengine_set_spot_light_intensity(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSpotLightIntensity { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_set_spot_light_range(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSpotLightRange { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_set_spot_light_inner_angle(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSpotLightInnerAngle { name, deg })
    });
}

#[op2(fast)]
pub fn bsengine_set_spot_light_outer_angle(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSpotLightOuterAngle { name, deg })
    });
}

#[op2(fast)]
pub fn bsengine_set_directional_light_color(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetDirectionalLightColor { name, r, g, b })
    });
}

#[op2(fast)]
pub fn bsengine_set_directional_light_ambient(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetDirectionalLightAmbient { name, r, g, b })
    });
}

#[op2(fast)]
pub fn bsengine_set_directional_light_direction(#[string] name: String, x: f32, y: f32, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetDirectionalLightDirection { name, x, y, z })
    });
}

#[op2(fast)]
pub fn bsengine_set_camera_fov(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCameraFov { name, deg })
    });
}

#[op2(fast)]
pub fn bsengine_set_camera_near(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCameraNear { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_set_camera_far(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCameraFar { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_set_damping(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetDamping { name, value })
    });
}

#[op2(fast)]
pub fn bsengine_play_animation(#[string] name: String, #[string] clip: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::PlayAnimation { name, clip })
    });
}

#[op2(fast)]
pub fn bsengine_pause_animation(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::PauseAnimation { name }));
}

#[op2(fast)]
pub fn bsengine_resume_animation(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ResumeAnimation { name }));
}

#[op2(fast)]
pub fn bsengine_reset_animation(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ResetAnimation { name }));
}

#[op2(fast)]
pub fn bsengine_set_animation_speed(#[string] name: String, speed: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAnimationSpeed { name, speed })
    });
}

#[op2(fast)]
pub fn bsengine_set_animation_looping(#[string] name: String, looping: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAnimationLooping { name, looping })
    });
}

#[op2(fast)]
pub fn bsengine_anim_set_trigger(#[string] name: String, #[string] trigger: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AsmSetTrigger { name, trigger })
    });
}

#[op2(fast)]
pub fn bsengine_anim_set_float(#[string] name: String, #[string] param: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AsmSetFloat { name, param, value })
    });
}

#[op2(fast)]
pub fn bsengine_anim_set_bool(#[string] name: String, #[string] param: String, value: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AsmSetBool { name, param, value })
    });
}

#[op2]
#[string]
pub fn bsengine_anim_get_state(#[string] name: String) -> String {
    ASM_STATE_SNAPSHOT.with(|s| s.borrow().get(&name).cloned().unwrap_or_default())
}

#[op2]
#[string]
pub fn bsengine_get_animation_clip(#[string] name: String) -> String {
    ANIMATION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(clip, _, _, _, _)| clip.clone())
            .unwrap_or_default()
    })
}

#[op2(fast)]
pub fn bsengine_get_animation_time(#[string] name: String) -> f32 {
    ANIMATION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, time, _, _, _)| *time)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_animation_speed(#[string] name: String) -> f32 {
    ANIMATION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, speed, _, _)| *speed)
            .unwrap_or(1.0)
    })
}

#[op2(fast)]
pub fn bsengine_is_animation_playing(#[string] name: String) -> bool {
    ANIMATION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, playing)| *playing)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_is_animation_looping(#[string] name: String) -> bool {
    ANIMATION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, looping, _)| *looping)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_set_lifetime(#[string] name: String, seconds: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetLifetime { name, seconds })
    });
}

#[op2(fast)]
pub fn bsengine_get_lifetime(#[string] name: String) -> f32 {
    LIFETIME_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_damage_shield(#[string] name: String, amount: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::DamageShield { name, amount })
    });
}

#[op2(fast)]
pub fn bsengine_restore_shield(#[string] name: String, amount: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::RestoreShield { name, amount })
    });
}

#[op2(fast)]
pub fn bsengine_set_max_shield(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetMaxShield { name, value })
    });
}

// ── Scald ────────────────────────────────────────────────────────────────────
// ── Scan ─────────────────────────────────────────────────────────────────────
// ── Scar ─────────────────────────────────────────────────────────────────────
// ── Scatter ──────────────────────────────────────────────────────────────────
// ── Scope ────────────────────────────────────────────────────────────────────
// ── Scorch ───────────────────────────────────────────────────────────────────
// ── Shear ────────────────────────────────────────────────────────────────────
// ── Shock ────────────────────────────────────────────────────────────────────
// ── Shrivel ──────────────────────────────────────────────────────────────────
// ── Shroud ───────────────────────────────────────────────────────────────────
// ── Shunt ────────────────────────────────────────────────────────────────────
// ── Spike ────────────────────────────────────────────────────────────────────
// ── Splinter ─────────────────────────────────────────────────────────────────
// ── Stagger ───────────────────────────────────────────────────────────────────
// ── Stake ─────────────────────────────────────────────────────────────────────
// ── Stalk ─────────────────────────────────────────────────────────────────────
// ── Stance ────────────────────────────────────────────────────────────────────
// ── Stat ──────────────────────────────────────────────────────────────────────
// ── Stealth ───────────────────────────────────────────────────────────────────
// ── Stomp ─────────────────────────────────────────────────────────────────────
// ── Stride ────────────────────────────────────────────────────────────────────
// ── Strife ────────────────────────────────────────────────────────────────────
// ── Stumble ───────────────────────────────────────────────────────────────────
// ── Sulk ─────────────────────────────────────────────────────────────────────
// ── Sunder ───────────────────────────────────────────────────────────────────
// ── Suppress ─────────────────────────────────────────────────────────────────
// ── Surge ────────────────────────────────────────────────────────────────────
// ── Surround ─────────────────────────────────────────────────────────────────
// ── Survive ──────────────────────────────────────────────────────────────────
// ── Swim ─────────────────────────────────────────────────────────────────────
// ── Taint ─────────────────────────────────────────────────────────────────────
// ── Tally ─────────────────────────────────────────────────────────────────────
// ── Talon ─────────────────────────────────────────────────────────────────────
// ── Taper ─────────────────────────────────────────────────────────────────────
// ── Taunt ─────────────────────────────────────────────────────────────────────
// ── Thaw ──────────────────────────────────────────────────────────────────────
// ── Trample ──────────────────────────────────────────────────────────────────
// ── Trance ───────────────────────────────────────────────────────────────────
// ── Tranquil ─────────────────────────────────────────────────────────────────
// ── Transfix ─────────────────────────────────────────────────────────────────
// ── Tremble ──────────────────────────────────────────────────────────────────
// ── Tremor ───────────────────────────────────────────────────────────────────
// ── Trove ────────────────────────────────────────────────────────────────────
// ── Tusk ─────────────────────────────────────────────────────────────────────
// ── Unrest ───────────────────────────────────────────────────────────────────
// ── Upkeep ───────────────────────────────────────────────────────────────────
// ── Urge ─────────────────────────────────────────────────────────────────────
// ── Venom ────────────────────────────────────────────────────────────────────
// ── Vex ──────────────────────────────────────────────────────────────────────
// ── Vigor ────────────────────────────────────────────────────────────────────
// ── Vile ─────────────────────────────────────────────────────────────────────
// ── Void ─────────────────────────────────────────────────────────────────────
// ── Venture ───────────────────────────────────────────────────────────────────
// ── Verge ─────────────────────────────────────────────────────────────────────
// ── Verify ────────────────────────────────────────────────────────────────────
// ── Verily ────────────────────────────────────────────────────────────────────
// ── Vermin ────────────────────────────────────────────────────────────────────
// ── Vernal ────────────────────────────────────────────────────────────────────
// ── Verse ─────────────────────────────────────────────────────────────────────
// ── Vertex ────────────────────────────────────────────────────────────────────
// ── Verve ─────────────────────────────────────────────────────────────────────
// ── Vest ──────────────────────────────────────────────────────────────────────
// ── Vice ──────────────────────────────────────────────────────────────────────
// ── Vim ───────────────────────────────────────────────────────────────────────
// ── Viper ─────────────────────────────────────────────────────────────────────
// ── Viral ─────────────────────────────────────────────────────────────────────
// ── Visit ─────────────────────────────────────────────────────────────────────
// ── Vista ─────────────────────────────────────────────────────────────────────
// ── Vibrate ──────────────────────────────────────────────────────────────────
// ── Viewport ─────────────────────────────────────────────────────────────────
// ── Vision ───────────────────────────────────────────────────────────────────
// ── VolumetricLight ──────────────────────────────────────────────────────────
// ── Volley ────────────────────────────────────────────────────────────────────
// ── Vortex ───────────────────────────────────────────────────────────────────
// ── Vow ──────────────────────────────────────────────────────────────────────
// ── Vulnerable ───────────────────────────────────────────────────────────────
// ── Vulture ───────────────────────────────────────────────────────────────────
// ── Wage ──────────────────────────────────────────────────────────────────────
// ── Wager ─────────────────────────────────────────────────────────────────────
// ── Wail ──────────────────────────────────────────────────────────────────────
// ── Wake ──────────────────────────────────────────────────────────────────────
// ── Walk ──────────────────────────────────────────────────────────────────────
// ── Wall ──────────────────────────────────────────────────────────────────────
// ── Waltz ─────────────────────────────────────────────────────────────────────
// ── Wand ──────────────────────────────────────────────────────────────────────
// ── Wane ──────────────────────────────────────────────────────────────────────
// ── Wangle ────────────────────────────────────────────────────────────────────
// ── Want ──────────────────────────────────────────────────────────────────────
// ── Wanton ────────────────────────────────────────────────────────────────────
// ── Ward ──────────────────────────────────────────────────────────────────────
// ── Warm ──────────────────────────────────────────────────────────────────────
// ── Warp ──────────────────────────────────────────────────────────────────────
// ── Warn ─────────────────────────────────────────────────────────────────────
// ── Wary ─────────────────────────────────────────────────────────────────────
// ── Wash ─────────────────────────────────────────────────────────────────────
// ── Wasp ─────────────────────────────────────────────────────────────────────
// ── Waste ────────────────────────────────────────────────────────────────────
// ── WaterBody ────────────────────────────────────────────────────────────────
// ── Wave ─────────────────────────────────────────────────────────────────────
// ── Waver ────────────────────────────────────────────────────────────────────
// ── Wax ──────────────────────────────────────────────────────────────────────
// ── Way ──────────────────────────────────────────────────────────────────────
// ── Weal ─────────────────────────────────────────────────────────────────────
// ── Weary ────────────────────────────────────────────────────────────────────
// ── Weather ──────────────────────────────────────────────────────────────────
// ── Weave ────────────────────────────────────────────────────────────────────
// ── Weasel ───────────────────────────────────────────────────────────────────
// ── Web ──────────────────────────────────────────────────────────────────────
// ── Wed ──────────────────────────────────────────────────────────────────────
// ── Wedge ────────────────────────────────────────────────────────────────────
// ── Wee ──────────────────────────────────────────────────────────────────────
// ── Weed ─────────────────────────────────────────────────────────────────────
// ── Weedy ────────────────────────────────────────────────────────────────────
// ── Weep ─────────────────────────────────────────────────────────────────────
// ── Weft ─────────────────────────────────────────────────────────────────────
// ── Weigh ────────────────────────────────────────────────────────────────────
// ── Weight ───────────────────────────────────────────────────────────────────
// ── Weird ────────────────────────────────────────────────────────────────────
// ── Weld ─────────────────────────────────────────────────────────────────────
// ── Welder ───────────────────────────────────────────────────────────────────
// ── Welkin ───────────────────────────────────────────────────────────────────
// ── Well ─────────────────────────────────────────────────────────────────────
// ── Welly ────────────────────────────────────────────────────────────────────
// ── Welp ─────────────────────────────────────────────────────────────────────
// ── Welt ─────────────────────────────────────────────────────────────────────
// ── Wend ─────────────────────────────────────────────────────────────────────
// ── Whiff ────────────────────────────────────────────────────────────────────
// ── Whim ─────────────────────────────────────────────────────────────────────
// ── Whip ─────────────────────────────────────────────────────────────────────
// ── Whirl ────────────────────────────────────────────────────────────────────
// ── Whisk ────────────────────────────────────────────────────────────────────
// ── Wick ─────────────────────────────────────────────────────────────────────
// ── Wicker ───────────────────────────────────────────────────────────────────
// ── Wig ──────────────────────────────────────────────────────────────────────
// ── Wild ─────────────────────────────────────────────────────────────────────
// ── Wilder ───────────────────────────────────────────────────────────────────
// ── Wile ─────────────────────────────────────────────────────────────────────
// ── Wiles ────────────────────────────────────────────────────────────────────
// ── Will ─────────────────────────────────────────────────────────────────────
// ── Willow ───────────────────────────────────────────────────────────────────
// ── Wilt ─────────────────────────────────────────────────────────────────────
// ── Wily ─────────────────────────────────────────────────────────────────────
// ── Wimp ─────────────────────────────────────────────────────────────────────
// ── Wimple ───────────────────────────────────────────────────────────────────
// ── Win ──────────────────────────────────────────────────────────────────────
// ── Wince ────────────────────────────────────────────────────────────────────
// ── Winch ────────────────────────────────────────────────────────────────────
// ── Winder ───────────────────────────────────────────────────────────────────
// ── Windfall ─────────────────────────────────────────────────────────────────
// ── Windup ───────────────────────────────────────────────────────────────────
// ── Wine ─────────────────────────────────────────────────────────────────────
// ── Wing ─────────────────────────────────────────────────────────────────────
// ── Wink ─────────────────────────────────────────────────────────────────────
// ── Wino ─────────────────────────────────────────────────────────────────────
// ── Winsome ──────────────────────────────────────────────────────────────────
// ── Wintry ───────────────────────────────────────────────────────────────────
// ── Wire ─────────────────────────────────────────────────────────────────────
// ── Wise ─────────────────────────────────────────────────────────────────────
// ── Wish ─────────────────────────────────────────────────────────────────────
// ── Wisp ─────────────────────────────────────────────────────────────────────
// ── Wispy ────────────────────────────────────────────────────────────────────
// ── Wist ─────────────────────────────────────────────────────────────────────
// ── Wistful ──────────────────────────────────────────────────────────────────
// ── Wit ──────────────────────────────────────────────────────────────────────
// ── Witch ─────────────────────────────────────────────────────────────────────
// ── Witless ───────────────────────────────────────────────────────────────────
// ── Witty ─────────────────────────────────────────────────────────────────────
// ── Wiz ───────────────────────────────────────────────────────────────────────
// ── Woe ───────────────────────────────────────────────────────────────────────
// ── Woeful ────────────────────────────────────────────────────────────────────
// ── Wok ───────────────────────────────────────────────────────────────────────
// ── Woke ──────────────────────────────────────────────────────────────────────
// ── Woken ─────────────────────────────────────────────────────────────────────
// ── Wold ──────────────────────────────────────────────────────────────────────
// ── Wolf ──────────────────────────────────────────────────────────────────────
// ── Womb ──────────────────────────────────────────────────────────────────────
// ── Wombat ────────────────────────────────────────────────────────────────────
// ── Women ─────────────────────────────────────────────────────────────────────
// ── Won ───────────────────────────────────────────────────────────────────────
// ── Wonder ────────────────────────────────────────────────────────────────────
// ── Wondrous ──────────────────────────────────────────────────────────────────
// ── Wonk ──────────────────────────────────────────────────────────────────────
// ── Wonky ──────────────────────────────────────────────────────────────────────
// ── Wont ──────────────────────────────────────────────────────────────────────
// ── Woo ──────────────────────────────────────────────────────────────────────
// ── Wood ──────────────────────────────────────────────────────────────────────
// ── Woodsy ──────────────────────────────────────────────────────────────────────
// ── Wooer ──────────────────────────────────────────────────────────────────────
// ── Woof ──────────────────────────────────────────────────────────────────────
// ── Wool ──────────────────────────────────────────────────────────────────────
// ── Woolly ──────────────────────────────────────────────────────────────────────
// ── Woozy ──────────────────────────────────────────────────────────────────────
// ── Wordy ──────────────────────────────────────────────────────────────────────
// ── Wore ──────────────────────────────────────────────────────────────────────
// ── Worm ──────────────────────────────────────────────────────────────────────
// ── Worn ──────────────────────────────────────────────────────────────────────
// ── Worry ──────────────────────────────────────────────────────────────────────
// ── Worse ──────────────────────────────────────────────────────────────────────
// ── Worst ──────────────────────────────────────────────────────────────────────
// ── Wort ──────────────────────────────────────────────────────────────────────
// ── Worthy ──────────────────────────────────────────────────────────────────────
// ── Wound ──────────────────────────────────────────────────────────────────────
// ── Wraith ──────────────────────────────────────────────────────────────────────
// ── Wrangle ──────────────────────────────────────────────────────────────────────
// ── Wrap ─────────────────────────────────────────────────────────────────────
// ── Wrath ────────────────────────────────────────────────────────────────────
// ── Wrathful ─────────────────────────────────────────────────────────────────
// ── Wreck ────────────────────────────────────────────────────────────────────
// ── Wrecker ──────────────────────────────────────────────────────────────────
// ── Wren ─────────────────────────────────────────────────────────────────────
// ── Wrench ───────────────────────────────────────────────────────────────────
// ── Wrest ────────────────────────────────────────────────────────────────────
// ── Wrestle ──────────────────────────────────────────────────────────────────
// ── Wretch ───────────────────────────────────────────────────────────────────
// ── Wretched ─────────────────────────────────────────────────────────────────
// ── Wriggle ───────────────────────────────────────────────────────────────────
// ── Wring ─────────────────────────────────────────────────────────────────────
// ── Wrinkle ───────────────────────────────────────────────────────────────────
// ── Wrist ─────────────────────────────────────────────────────────────────────
// ── Write ─────────────────────────────────────────────────────────────────────
// ── Writhe ───────────────────────────────────────────────────────────────────
// ── Wrong ────────────────────────────────────────────────────────────────────
// ── Wrongly ──────────────────────────────────────────────────────────────────
// ── Wrote ────────────────────────────────────────────────────────────────────
// ── Wroth ────────────────────────────────────────────────────────────────────
// ── Wrung ────────────────────────────────────────────────────────────────────
// ── Wry ──────────────────────────────────────────────────────────────────────
// ── Xray ─────────────────────────────────────────────────────────────────────
// ── Yak ──────────────────────────────────────────────────────────────────────
// ── Yam ──────────────────────────────────────────────────────────────────────
// ── Yang ─────────────────────────────────────────────────────────────────────
// ── Yank ─────────────────────────────────────────────────────────────────────
// ── Yap ──────────────────────────────────────────────────────────────────────
// ── Yard ─────────────────────────────────────────────────────────────────────
// ── Yare ─────────────────────────────────────────────────────────────────────
// ── Yule ─────────────────────────────────────────────────────────────────────
// ── Yum ──────────────────────────────────────────────────────────────────────
// ── Yummy ────────────────────────────────────────────────────────────────────
// ── Yup ──────────────────────────────────────────────────────────────────────
// ── Yurt ─────────────────────────────────────────────────────────────────────
// ── Zafu ─────────────────────────────────────────────────────────────────────
// ── Zag ──────────────────────────────────────────────────────────────────────
// ── Zaibatsu ─────────────────────────────────────────────────────────────────
// ── Zakat ────────────────────────────────────────────────────────────────────
// ── Zamia ────────────────────────────────────────────────────────────────────
// ── Zanily ───────────────────────────────────────────────────────────────────
// ── Zaniness ─────────────────────────────────────────────────────────────────
// ── Zany ─────────────────────────────────────────────────────────────────────
// ── Zap ──────────────────────────────────────────────────────────────────────
// ── Zapper ───────────────────────────────────────────────────────────────────
// ── Zappy ────────────────────────────────────────────────────────────────────
// ── Zeal ─────────────────────────────────────────────────────────────────────
// ── Zealot ───────────────────────────────────────────────────────────────────
// ── Zealotry ─────────────────────────────────────────────────────────────────
// ── Zealous ──────────────────────────────────────────────────────────────────
// ── Zeatin ───────────────────────────────────────────────────────────────────
// ── Zeaxanthin ───────────────────────────────────────────────────────────────
// ── Zebec ─────────────────────────────────────────────────────────────────────
// ── Zebra ─────────────────────────────────────────────────────────────────────
// ── Zebrafish ─────────────────────────────────────────────────────────────────
// ── Zebrine ───────────────────────────────────────────────────────────────────
// ── Zebroid ───────────────────────────────────────────────────────────────────
// ── Zebu ──────────────────────────────────────────────────────────────────────
// ── Zechin ────────────────────────────────────────────────────────────────────
// ── Zed ───────────────────────────────────────────────────────────────────────
// ── Zeekoe ────────────────────────────────────────────────────────────────────
// ── Zein ──────────────────────────────────────────────────────────────────────
// ── Zeitgeber ─────────────────────────────────────────────────────────────────
// ── Zeitgeist ─────────────────────────────────────────────────────────────────
// ── Zek ───────────────────────────────────────────────────────────────────────
// ── Zelkova ───────────────────────────────────────────────────────────────────
// ── Zemstvo ───────────────────────────────────────────────────────────────────
// ── Zen ───────────────────────────────────────────────────────────────────────
// ── Zenana ────────────────────────────────────────────────────────────────────
// ── Zendo ─────────────────────────────────────────────────────────────────────
// ── Zener ─────────────────────────────────────────────────────────────────────
// ── Zenith ────────────────────────────────────────────────────────────────────
// ── Zenithal ──────────────────────────────────────────────────────────────────
// ── Zeolite ───────────────────────────────────────────────────────────────────
// ── Zeolitic ──────────────────────────────────────────────────────────────────
// ── Zephyr ────────────────────────────────────────────────────────────────────
// ── Zeppelin ──────────────────────────────────────────────────────────────────
// ── Zerk ──────────────────────────────────────────────────────────────────────
// ── Zeroth ───────────────────────────────────────────────────────────────────
// ── Zester ───────────────────────────────────────────────────────────────────
// ── Zestful ──────────────────────────────────────────────────────────────────
// ── Zeta ─────────────────────────────────────────────────────────────────────
// ── Zetetic ──────────────────────────────────────────────────────────────────
// ── Zeugen ───────────────────────────────────────────────────────────────────
// ── Zeugma ───────────────────────────────────────────────────────────────────
// ── Zho ──────────────────────────────────────────────────────────────────────
// ── Zillion ───────────────────────────────────────────────────────────────────
// ── Zimb ──────────────────────────────────────────────────────────────────────
// ── Zinc ──────────────────────────────────────────────────────────────────────
// ── Zincate ───────────────────────────────────────────────────────────────────
// ── Zincite ───────────────────────────────────────────────────────────────────
// ── Zine ──────────────────────────────────────────────────────────────────────
// ── Zineb ─────────────────────────────────────────────────────────────────────
// ── Zinfandel ─────────────────────────────────────────────────────────────────
// ── Zing ─────────────────────────────────────────────────────────────────────
// ── Zinger ────────────────────────────────────────────────────────────────────
// ── Zink ──────────────────────────────────────────────────────────────────────
// ── Zinnia ────────────────────────────────────────────────────────────────────
// ── Zip ───────────────────────────────────────────────────────────────────────
// ── Zipper ────────────────────────────────────────────────────────────────────
// ── Zippier ───────────────────────────────────────────────────────────────────
// ── Zippy ─────────────────────────────────────────────────────────────────────
// ── Zircon ───────────────────────────────────────────────────────────────────
// ── Zirconia ─────────────────────────────────────────────────────────────────
// ── Zirconium ────────────────────────────────────────────────────────────────
// ── Zit ──────────────────────────────────────────────────────────────────────
// ── Zither ───────────────────────────────────────────────────────────────────
// ── Ziti ─────────────────────────────────────────────────────────────────────
// ── Zoanthropy ───────────────────────────────────────────────────────────────
// ── Zodiac ───────────────────────────────────────────────────────────────────
// ── Zombify ───────────────────────────────────────────────────────────────────
// ── Zonal ─────────────────────────────────────────────────────────────────────
// ── Zonate ────────────────────────────────────────────────────────────────────
// ── Zonation ──────────────────────────────────────────────────────────────────
// ── Zone ──────────────────────────────────────────────────────────────────────
// ── Zoner ─────────────────────────────────────────────────────────────────────
// ── Zoning ────────────────────────────────────────────────────────────────────
// ── Zonk ──────────────────────────────────────────────────────────────────────
// ── Zoo ───────────────────────────────────────────────────────────────────────
// ── Zoogenous ────────────────────────────────────────────────────────────────
// ── Zoogeography ─────────────────────────────────────────────────────────────
// ── Zooglea ───────────────────────────────────────────────────────────────────
// ── Zoography ─────────────────────────────────────────────────────────────────
// ── Zooid ─────────────────────────────────────────────────────────────────────
// ── Zookeeper ─────────────────────────────────────────────────────────────────
// ── Zoolatry ──────────────────────────────────────────────────────────────────
// ── Zoological ────────────────────────────────────────────────────────────────
// ── Zoologist ─────────────────────────────────────────────────────────────────
// ── Zoology ───────────────────────────────────────────────────────────────────
// ── Zoom ──────────────────────────────────────────────────────────────────────
// ── Zoometry ──────────────────────────────────────────────────────────────────
// ── Zoomorph ──────────────────────────────────────────────────────────────────
// ── Zoomorphic ────────────────────────────────────────────────────────────────
// ── Zoomorphism ───────────────────────────────────────────────────────────────
// ── Zoonosis ──────────────────────────────────────────────────────────────────
// ── Zoonotic ──────────────────────────────────────────────────────────────────
// ── Zoophagous ──────────────────────────────────────────────────────────────────
// ── Zoophile ──────────────────────────────────────────────────────────────────
// ── Zoophilia ──────────────────────────────────────────────────────────────────
// ── Zoophilous ──────────────────────────────────────────────────────────────────
// ── Zoophily ──────────────────────────────────────────────────────────────────
// ── Zoophyte ──────────────────────────────────────────────────────────────────
// ── Zooplankton ───────────────────────────────────────────────────────────────
// ── Zoosphere ─────────────────────────────────────────────────────────────────
// ── Zoosperm ──────────────────────────────────────────────────────────────────
// ── Zoospore ──────────────────────────────────────────────────────────────────
// ── Zootechnics ───────────────────────────────────────────────────────────────
// ── Zootomy ───────────────────────────────────────────────────────────────────
// ── Zooxanthella ──────────────────────────────────────────────────────────────
// ── Zoogamy ───────────────────────────────────────────────────────────────────
// ── Zoogenesis ────────────────────────────────────────────────────────────────
// ── Zoognomy ──────────────────────────────────────────────────────────────────
// ── Zoopathology ──────────────────────────────────────────────────────────────
// ── Zoophobia ─────────────────────────────────────────────────────────────────
// ── Zooscopy ──────────────────────────────────────────────────────────────────
// ── Zootherapy ────────────────────────────────────────────────────────────────
// ── Silence ──────────────────────────────────────────────────────────────────
// ── Siphon ───────────────────────────────────────────────────────────────────
// ── Slam ─────────────────────────────────────────────────────────────────────
// ── Slay ─────────────────────────────────────────────────────────────────────
// ── Slide ────────────────────────────────────────────────────────────────────
// ── Slime ────────────────────────────────────────────────────────────────────
// ── Slink ────────────────────────────────────────────────────────────────────
// ── SlowMo ───────────────────────────────────────────────────────────────────
// ── Smoke ────────────────────────────────────────────────────────────────────
// ── Snare ────────────────────────────────────────────────────────────────────
// ── Soak ─────────────────────────────────────────────────────────────────────
#[op2(fast)]
pub fn bsengine_get_shield(#[string] name: String) -> f32 {
    SHIELD_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(cur, _)| *cur).unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_max_shield(#[string] name: String) -> f32 {
    SHIELD_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, max)| *max).unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_shield_fraction(#[string] name: String) -> f32 {
    SHIELD_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(cur, max)| {
                if *max <= 0.0 {
                    0.0
                } else {
                    (*cur / *max).clamp(0.0, 1.0)
                }
            })
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_is_shield_depleted(#[string] name: String) -> bool {
    SHIELD_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(cur, _)| *cur <= 0.0)
            .unwrap_or(true)
    })
}

#[op2(fast)]
pub fn bsengine_reset_timer(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ResetTimer { name }));
}

#[op2(fast)]
pub fn bsengine_get_timer_elapsed(#[string] name: String) -> f32 {
    TIMER_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(elapsed, _, _, _, _)| *elapsed)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_timer_duration(#[string] name: String) -> f32 {
    TIMER_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, dur, _, _, _)| *dur)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_timer_fraction(#[string] name: String) -> f32 {
    TIMER_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, frac, _, _)| *frac)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_is_timer_finished(#[string] name: String) -> bool {
    TIMER_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, fin, _)| *fin)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_is_timer_just_finished(#[string] name: String) -> bool {
    TIMER_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, jf)| *jf)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_set_nav_destination(#[string] name: String, x: f32, y: f32, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetNavDestination { name, x, y, z })
    });
}

#[op2(fast)]
pub fn bsengine_clear_nav_destination(#[string] name: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::ClearNavDestination { name })
    });
}

#[op2(fast)]
pub fn bsengine_set_nav_speed(#[string] name: String, speed: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetNavSpeed { name, speed })
    });
}

#[op2(fast)]
pub fn bsengine_set_nav_angular_speed(#[string] name: String, speed: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetNavAngularSpeed { name, speed })
    });
}

#[op2(fast)]
pub fn bsengine_set_nav_stopping_distance(#[string] name: String, distance: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetNavStoppingDistance { name, distance })
    });
}

#[op2(fast)]
pub fn bsengine_set_nav_enabled(#[string] name: String, enabled: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetNavEnabled { name, enabled })
    });
}

#[op2(fast)]
pub fn bsengine_get_nav_speed(#[string] name: String) -> f32 {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(spd, _, _, _, _)| *spd)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_nav_angular_speed(#[string] name: String) -> f32 {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, asp, _, _, _)| *asp)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_nav_stopping_distance(#[string] name: String) -> f32 {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, sd, _, _)| *sd)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_is_nav_moving(#[string] name: String) -> bool {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, st, _)| *st == 1)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_has_nav_arrived(#[string] name: String) -> bool {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, st, _)| *st == 2)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_is_nav_idle(#[string] name: String) -> bool {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, st, _)| *st == 0)
            .unwrap_or(true)
    })
}

#[op2(fast)]
pub fn bsengine_nav_has_no_path(#[string] name: String) -> bool {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, st, _)| *st == 3)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_is_nav_enabled(#[string] name: String) -> bool {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, en)| *en)
            .unwrap_or(true)
    })
}

#[op2(fast)]
pub fn bsengine_navmesh_init(width: u32, depth: u32, cell_size: f32, ox: f32, oy: f32, oz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::NavmeshInit {
            width,
            depth,
            cell_size,
            origin_x: ox,
            origin_y: oy,
            origin_z: oz,
        })
    });
}

#[op2(fast)]
pub fn bsengine_navmesh_set_walkable(x: u32, z: u32, walkable: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::NavmeshSetWalkable { x, z, walkable })
    });
}

#[op2]
#[string]
pub fn bsengine_navmesh_get_state(#[string] name: String) -> String {
    NAV_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, st, _)| match st {
                1 => "moving",
                2 => "arrived",
                3 => "no_path",
                _ => "idle",
            })
            .unwrap_or("idle")
            .to_string()
    })
}

#[op2(fast)]
pub fn bsengine_save_game(#[string] path: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::SaveGame { path }));
}

#[op2(fast)]
pub fn bsengine_load_game(#[string] path: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::LoadGame { path }));
}

#[op2(fast)]
pub fn bsengine_material_set_shader(#[string] name: String, #[string] path: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCustomShader { name, path });
    });
}

#[op2(fast)]
pub fn bsengine_material_clear_shader(#[string] name: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::ClearCustomShader { name });
    });
}

#[op2(fast)]
pub fn bsengine_set_bloom_intensity(#[string] name: String, intensity: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetBloomIntensity { name, intensity })
    });
}

#[op2(fast)]
pub fn bsengine_set_bloom_threshold(#[string] name: String, threshold: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetBloomThreshold { name, threshold })
    });
}

#[op2(fast)]
pub fn bsengine_set_bloom_radius(#[string] name: String, radius: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetBloomRadius { name, radius })
    });
}

#[op2(fast)]
pub fn bsengine_set_bloom_softness(#[string] name: String, softness: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetBloomSoftness { name, softness })
    });
}

#[op2(fast)]
pub fn bsengine_set_bloom_enabled(#[string] name: String, enabled: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetBloomEnabled { name, enabled })
    });
}

#[op2(fast)]
pub fn bsengine_get_bloom_intensity(#[string] name: String) -> f32 {
    BLOOM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(intensity, _, _, _, _)| *intensity)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_bloom_threshold(#[string] name: String) -> f32 {
    BLOOM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, threshold, _, _, _)| *threshold)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_bloom_radius(#[string] name: String) -> f32 {
    BLOOM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, radius, _, _)| *radius)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_bloom_softness(#[string] name: String) -> f32 {
    BLOOM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, softness, _)| *softness)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_is_bloom_enabled(#[string] name: String) -> bool {
    BLOOM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, en)| *en)
            .unwrap_or(true)
    })
}

#[op2(fast)]
pub fn bsengine_set_ao_radius(#[string] name: String, radius: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAoRadius { name, radius })
    });
}

#[op2(fast)]
pub fn bsengine_set_ao_bias(#[string] name: String, bias: f32) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::SetAoBias { name, bias }));
}

#[op2(fast)]
pub fn bsengine_set_ao_intensity(#[string] name: String, intensity: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAoIntensity { name, intensity })
    });
}

#[op2(fast)]
pub fn bsengine_set_ao_sample_count(#[string] name: String, count: u32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAoSampleCount { name, count })
    });
}

#[op2(fast)]
pub fn bsengine_set_ao_enabled(#[string] name: String, enabled: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAoEnabled { name, enabled })
    });
}

#[op2(fast)]
pub fn bsengine_get_ao_radius(#[string] name: String) -> f32 {
    AMBIENT_OCCLUSION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(radius, _, _, _, _)| *radius)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_ao_bias(#[string] name: String) -> f32 {
    AMBIENT_OCCLUSION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, bias, _, _, _)| *bias)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_ao_intensity(#[string] name: String) -> f32 {
    AMBIENT_OCCLUSION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, intensity, _, _)| *intensity)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_ao_sample_count(#[string] name: String) -> u32 {
    AMBIENT_OCCLUSION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, count, _)| *count)
            .unwrap_or(0)
    })
}

#[op2(fast)]
pub fn bsengine_is_ao_enabled(#[string] name: String) -> bool {
    AMBIENT_OCCLUSION_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, en)| *en)
            .unwrap_or(true)
    })
}

#[op2(fast)]
pub fn bsengine_set_tone_map_mode(#[string] name: String, mode: u32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetToneMapMode { name, mode })
    });
}

#[op2(fast)]
pub fn bsengine_set_tone_map_exposure(#[string] name: String, exposure: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetToneMapExposure { name, exposure })
    });
}

#[op2(fast)]
pub fn bsengine_set_tone_map_enabled(#[string] name: String, enabled: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetToneMapEnabled { name, enabled })
    });
}

#[op2(fast)]
pub fn bsengine_get_tone_map_mode(#[string] name: String) -> u32 {
    TONE_MAP_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(m, _, _)| *m).unwrap_or(0))
}

#[op2(fast)]
pub fn bsengine_get_tone_map_exposure(#[string] name: String) -> f32 {
    TONE_MAP_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, e, _)| *e).unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_is_tone_map_enabled(#[string] name: String) -> bool {
    TONE_MAP_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, _, en)| *en).unwrap_or(true))
}

#[op2(fast)]
pub fn bsengine_set_tween_duration(#[string] name: String, duration: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetTweenDuration { name, duration })
    });
}

#[op2(fast)]
pub fn bsengine_set_tween_easing(#[string] name: String, easing: u32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetTweenEasing { name, easing })
    });
}

#[op2(fast)]
pub fn bsengine_set_tween_repeat(#[string] name: String, repeat: u32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetTweenRepeat { name, repeat })
    });
}

#[op2(fast)]
pub fn bsengine_set_tween_elapsed(#[string] name: String, elapsed: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetTweenElapsed { name, elapsed })
    });
}

#[op2(fast)]
pub fn bsengine_get_tween_target_type(#[string] name: String) -> u32 {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(tt, _, _, _, _, _, _)| *tt)
            .unwrap_or(0)
    })
}

#[op2(fast)]
pub fn bsengine_get_tween_duration(#[string] name: String) -> f32 {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, d, _, _, _, _, _)| *d)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_tween_easing(#[string] name: String) -> u32 {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, e, _, _, _, _)| *e)
            .unwrap_or(0)
    })
}

#[op2(fast)]
pub fn bsengine_get_tween_repeat(#[string] name: String) -> u32 {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, r, _, _, _)| *r)
            .unwrap_or(0)
    })
}

#[op2(fast)]
pub fn bsengine_get_tween_elapsed(#[string] name: String) -> f32 {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, el, _, _)| *el)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_tween_progress(#[string] name: String) -> f32 {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, d, _, _, el, _, _)| {
                if *d > 0.0 {
                    (el / d).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_is_tween_finished(#[string] name: String) -> bool {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, _, fin, _)| *fin)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_is_tween_reversed(#[string] name: String) -> bool {
    TWEEN_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, _, _, rev)| *rev)
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_set_follow_target(#[string] name: String, #[string] target: String) {
    COMMAND_BUFFER.with(|b| {
        b.borrow_mut()
            .push(ScriptCommand::SetFollowTarget { name, target });
    });
}

#[op2(fast)]
pub fn bsengine_set_follow_offset(#[string] name: String, x: f32, y: f32, z: f32) {
    COMMAND_BUFFER.with(|b| {
        b.borrow_mut()
            .push(ScriptCommand::SetFollowOffset { name, x, y, z });
    });
}

#[op2(fast)]
pub fn bsengine_set_follow_speed(#[string] name: String, speed: f32) {
    COMMAND_BUFFER.with(|b| {
        b.borrow_mut()
            .push(ScriptCommand::SetFollowSpeed { name, speed });
    });
}

#[op2]
#[string]
pub fn bsengine_get_follow_target(#[string] name: String) -> String {
    FOLLOW_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(t, _, _, _, _)| format!("\"{t}\""))
            .unwrap_or_else(|| "null".to_string())
    })
}

#[op2(fast)]
pub fn bsengine_get_follow_offset_x(#[string] name: String) -> f32 {
    FOLLOW_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, x, _, _, _)| *x)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_follow_offset_y(#[string] name: String) -> f32 {
    FOLLOW_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, y, _, _)| *y)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_follow_offset_z(#[string] name: String) -> f32 {
    FOLLOW_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, z, _)| *z)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_get_follow_speed(#[string] name: String) -> f32 {
    FOLLOW_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, _, _, sp)| *sp)
            .unwrap_or(0.0)
    })
}

#[op2(fast)]
pub fn bsengine_set_look_at_target(#[string] name: String, #[string] target: String) {
    COMMAND_BUFFER.with(|b| {
        b.borrow_mut()
            .push(ScriptCommand::SetLookAtTarget { name, target });
    });
}

#[op2(fast)]
pub fn bsengine_set_look_at_up(#[string] name: String, x: f32, y: f32, z: f32) {
    COMMAND_BUFFER.with(|b| {
        b.borrow_mut()
            .push(ScriptCommand::SetLookAtUp { name, x, y, z });
    });
}

#[op2]
#[string]
pub fn bsengine_get_look_at_target(#[string] name: String) -> String {
    LOOK_AT_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(t, _, _, _)| format!("\"{t}\""))
            .unwrap_or_else(|| "null".to_string())
    })
}

#[op2(fast)]
pub fn bsengine_get_look_at_up_x(#[string] name: String) -> f32 {
    LOOK_AT_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, x, _, _)| *x).unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_look_at_up_y(#[string] name: String) -> f32 {
    LOOK_AT_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, _, y, _)| *y).unwrap_or(1.0))
}

#[op2(fast)]
pub fn bsengine_get_look_at_up_z(#[string] name: String) -> f32 {
    LOOK_AT_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, _, _, z)| *z).unwrap_or(0.0))
}

// --- Burn ---

// --- Bleed ---

// --- Poison ---

// --- Freeze ---

// --- Blind ---

// --- Charm ---

// --- Confuse ---

// --- Cripple ---

// --- Daze ---

// --- Disarm ---

// --- Concuss ---

// --- Corrosion ---

// --- Curse ---

// --- Dread ---

// --- Doom ---

// --- Demoralize ---

// --- Dodge ---

// --- Drain ---

// --- Empower ---

// --- Enervate ---

// --- Entangle ---

// --- Expose ---

// ── Exhaustion ──────────────────────────────────────────────────────────────

// ── Fear ────────────────────────────────────────────────────────────────────

// ── Fracture ─────────────────────────────────────────────────────────────────

// ── Frostbite ────────────────────────────────────────────────────────────────

// ── Fury ─────────────────────────────────────────────────────────────────────

// ── Galvanize ────────────────────────────────────────────────────────────────

// ── Haste ────────────────────────────────────────────────────────────────────

// ── Havoc ────────────────────────────────────────────────────────────────────

// ── Haze ─────────────────────────────────────────────────────────────────────

// ── Heat ─────────────────────────────────────────────────────────────────────

// ── Hex ──────────────────────────────────────────────────────────────────────

// ── Hobble ───────────────────────────────────────────────────────────────────

// ── Ignite ────────────────────────────────────────────────────────────────────

// ── Imbue ─────────────────────────────────────────────────────────────────────

// ── Immune ────────────────────────────────────────────────────────────────────

// ── Impact ────────────────────────────────────────────────────────────────────

// ── Intercept ─────────────────────────────────────────────────────────────────

// ── Interrupt ─────────────────────────────────────────────────────────────────

// ── Invincible ───────────────────────────────────────────────────────────────

// ── Isolate ───────────────────────────────────────────────────────────────────

// ── Jeer ─────────────────────────────────────────────────────────────────────

// ── Jetpack ───────────────────────────────────────────────────────────────────

// ── Jolt ─────────────────────────────────────────────────────────────────────

// ── Jostle ────────────────────────────────────────────────────────────────────

// ── Juke ─────────────────────────────────────────────────────────────────────

// ── Kneel ────────────────────────────────────────────────────────────────────

// ── Knit ─────────────────────────────────────────────────────────────────────

// ── Lacerate ─────────────────────────────────────────────────────────────────

// ── Laden ─────────────────────────────────────────────────────────────────────

// ── Lament ───────────────────────────────────────────────────────────────────

// ── Lance ─────────────────────────────────────────────────────────────────────

// ── Lapse ─────────────────────────────────────────────────────────────────────

// ── Lash ─────────────────────────────────────────────────────────────────────

// ── Latch ────────────────────────────────────────────────────────────────────

// ── Ledge ────────────────────────────────────────────────────────────────────

// ── Leech ────────────────────────────────────────────────────────────────────

// ── Lunge ────────────────────────────────────────────────────────────────────

// ── Lure ─────────────────────────────────────────────────────────────────────

// ── Lurk ─────────────────────────────────────────────────────────────────────

// ── Magnet ────────────────────────────────────────────────────────────────────

// ── Maim ──────────────────────────────────────────────────────────────────────

// ── Malice ────────────────────────────────────────────────────────────────────

// ── Mark ──────────────────────────────────────────────────────────────────────

// ── Melee ─────────────────────────────────────────────────────────────────────

// ── Mend ──────────────────────────────────────────────────────────────────────

// ── Merge ─────────────────────────────────────────────────────────────────────

// ── Mesh ──────────────────────────────────────────────────────────────────────

// ── Minimap ───────────────────────────────────────────────────────────────────

// ── Mirage ────────────────────────────────────────────────────────────────────

// ── Momentum ──────────────────────────────────────────────────────────────────

// ── Morale ────────────────────────────────────────────────────────────────────

// ── Morph ─────────────────────────────────────────────────────────────────────

// ── Mount ─────────────────────────────────────────────────────────────────────

// ── Muffle ────────────────────────────────────────────────────────────────────

// ── NetworkId ─────────────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn bsengine_get_network_id(#[string] name: String) -> String {
    NETWORK_ID_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(id, _, _)| id.clone())
            .unwrap_or_default()
    })
}

#[op2(fast)]
pub fn bsengine_get_network_authority(#[string] name: String) -> u32 {
    NETWORK_ID_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(_, auth, _)| *auth).unwrap_or(0))
}

#[op2]
#[string]
pub fn bsengine_get_network_peer_id(#[string] name: String) -> String {
    NETWORK_ID_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, _, peer)| peer.clone())
            .unwrap_or_default()
    })
}

#[op2(fast)]
pub fn bsengine_is_network_replicated(#[string] name: String) -> bool {
    NETWORK_ID_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(_, auth, _)| *auth != 2)
            .unwrap_or(false)
    })
}

// ── Network session ───────────────────────────────────────────────────────────

#[op2(fast)]
pub fn bsengine_network_start_server(port: u32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::NetworkStartServer { port: port as u16 });
    });
}

#[op2(fast)]
pub fn bsengine_network_connect(#[string] host: String, port: u32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::NetworkConnect {
            host,
            port: port as u16,
        });
    });
}

#[op2(fast)]
pub fn bsengine_network_disconnect() {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::NetworkDisconnect);
    });
}

#[op2(fast)]
pub fn bsengine_network_is_server() -> bool {
    NETWORK_STATE_SNAPSHOT.with(|s| s.borrow().0)
}

#[op2(fast)]
pub fn bsengine_network_is_connected() -> bool {
    NETWORK_STATE_SNAPSHOT.with(|s| s.borrow().1)
}

#[op2]
#[string]
pub fn bsengine_network_get_my_peer_id() -> String {
    NETWORK_STATE_SNAPSHOT.with(|s| s.borrow().2.to_string())
}

#[op2(fast)]
pub fn bsengine_network_get_peer_count() -> u32 {
    NETWORK_STATE_SNAPSHOT.with(|s| s.borrow().3)
}

// ── Nimble ────────────────────────────────────────────────────────────────────

// ── Notice ────────────────────────────────────────────────────────────────────

// ── Nourish ───────────────────────────────────────────────────────────────────

// ── Nova ──────────────────────────────────────────────────────────────────────

// ── Npc ───────────────────────────────────────────────────────────────────────

// ── Nullify ───────────────────────────────────────────────────────────────────

// ── Numb ──────────────────────────────────────────────────────────────────────

// ── Obstacle ──────────────────────────────────────────────────────────────────

// ── Omen ──────────────────────────────────────────────────────────────────────

// ── Orbit ─────────────────────────────────────────────────────────────────────

// ── Ordeal ────────────────────────────────────────────────────────────────────

// ── Oscillate ─────────────────────────────────────────────────────────────────

// ── Outlast ───────────────────────────────────────────────────────────────────

// ── Overflow ──────────────────────────────────────────────────────────────────

// ── Overheat ──────────────────────────────────────────────────────────────────

// ── Overload ──────────────────────────────────────────────────────────────────

// ── Overpower ─────────────────────────────────────────────────────────────────

// ── Overshield ────────────────────────────────────────────────────────────────

// ── Quest ─────────────────────────────────────────────────────────────────────

// ── Radar ─────────────────────────────────────────────────────────────────────

// ── Rage ──────────────────────────────────────────────────────────────────────

// ── Rally ─────────────────────────────────────────────────────────────────────

// ── Rampage ───────────────────────────────────────────────────────────────────

// ── Ravage ────────────────────────────────────────────────────────────────────

// ── Reave ─────────────────────────────────────────────────────────────────────

// ── Rebound ───────────────────────────────────────────────────────────────────

// ── Recharge ──────────────────────────────────────────────────────────────────

// ── Reckless ──────────────────────────────────────────────────────────────────

// ── Recluse ───────────────────────────────────────────────────────────────────

// ── Recoil ────────────────────────────────────────────────────────────────────

// ── Reflect ───────────────────────────────────────────────────────────────────

// ── Reflex ────────────────────────────────────────────────────────────────────

// ── Repel ─────────────────────────────────────────────────────────────────────

// ── Repose ────────────────────────────────────────────────────────────────────

// ── Respawn ───────────────────────────────────────────────────────────────────

// ── Retaliate ─────────────────────────────────────────────────────────────────

// ── Revenge ───────────────────────────────────────────────────────────────────

// ── Reveal ────────────────────────────────────────────────────────────────────

// ── Revive ────────────────────────────────────────────────────────────────────

// ── Ricochet ──────────────────────────────────────────────────────────────────

// ── Rifle ─────────────────────────────────────────────────────────────────────

// ── Rot ───────────────────────────────────────────────────────────────────────

// ── Rout ──────────────────────────────────────────────────────────────────────

// ── Rupture ───────────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn bsengine_look_at(#[string] name: String, tx: f32, ty: f32, tz: f32) {
    let origin = TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(pos, _, _)| *pos));
    if let Some(pos) = origin {
        let dir = Vec3::new(tx - pos.x, ty - pos.y, tz - pos.z);
        if dir.length_squared() < 1e-10 {
            return;
        }
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize());
        COMMAND_BUFFER.with(|c| {
            c.borrow_mut().push(ScriptCommand::SetRotation {
                name,
                rx: rot.x,
                ry: rot.y,
                rz: rot.z,
                rw: rot.w,
            });
        });
    }
}

#[op2(fast)]
pub fn bsengine_get_time() -> f32 {
    TIME_ELAPSED_SNAPSHOT.with(|s| *s.borrow())
}

#[op2(fast)]
pub fn bsengine_get_delta_time() -> f32 {
    TIME_DELTA_SNAPSHOT.with(|s| *s.borrow())
}

#[op2]
#[serde]
pub fn bsengine_get_screen_size() -> Vec<u32> {
    SCREEN_SIZE_SNAPSHOT.with(|s| {
        let (w, h) = *s.borrow();
        vec![w, h]
    })
}

#[op2(fast)]
pub fn bsengine_set_parent(#[string] child: String, #[string] parent: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetParent { child, parent });
    });
}

#[op2(fast)]
pub fn bsengine_clear_parent(#[string] child: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::ClearParent { child });
    });
}

#[op2]
#[string]
pub fn bsengine_get_parent(#[string] name: String) -> String {
    PARENT_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .cloned()
            .map(|p| format!("\"{p}\""))
            .unwrap_or_else(|| "null".to_string())
    })
}

#[op2]
#[string]
pub fn bsengine_get_children(#[string] name: String) -> String {
    CHILDREN_SNAPSHOT.with(|s| {
        serde_json::to_string(s.borrow().get(&name).unwrap_or(&Vec::new()))
            .unwrap_or_else(|_| "[]".to_string())
    })
}

#[op2]
#[serde]
pub fn bsengine_get_velocity(#[string] name: String) -> Option<Vec<f32>> {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map(|v| vec![v.x, v.y, v.z]))
}

#[op2]
#[serde]
pub fn bsengine_get_linear_speed(#[string] name: String) -> Option<Vec<f32>> {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map(|v| vec![v.length()]))
}

#[op2(fast)]
pub fn bsengine_get_velocity_x(#[string] name: String) -> f32 {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.x))
}

#[op2(fast)]
pub fn bsengine_get_velocity_y(#[string] name: String) -> f32 {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.y))
}

#[op2(fast)]
pub fn bsengine_get_velocity_z(#[string] name: String) -> f32 {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.z))
}

#[op2(fast)]
pub fn bsengine_add_impulse(#[string] name: String, fx: f32, fy: f32, fz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddImpulse { name, fx, fy, fz });
    });
}

#[op2(fast)]
pub fn bsengine_apply_impulse_at_point(
    #[string] name: String,
    fx: f32,
    fy: f32,
    fz: f32,
    px: f32,
    py: f32,
    pz: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddImpulseAtPoint {
            name,
            fx,
            fy,
            fz,
            px,
            py,
            pz,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_force(#[string] name: String, fx: f32, fy: f32, fz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddForce { name, fx, fy, fz });
    });
}

#[op2(fast)]
pub fn bsengine_add_force_at_point(
    #[string] name: String,
    fx: f32,
    fy: f32,
    fz: f32,
    px: f32,
    py: f32,
    pz: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddForceAtPoint {
            name,
            fx,
            fy,
            fz,
            px,
            py,
            pz,
        });
    });
}

#[op2(fast)]
pub fn bsengine_reset_forces(#[string] name: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::ResetForces { name });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity_x(#[string] name: String, vx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocityX { name, vx });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity_y(#[string] name: String, vy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocityY { name, vy });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity_z(#[string] name: String, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocityZ { name, vz });
    });
}

#[op2(fast)]
pub fn bsengine_get_gravity() -> f32 {
    GRAVITY_SNAPSHOT.with(|s| *s.borrow())
}

#[op2(fast)]
pub fn bsengine_set_gravity(magnitude: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetGravity { magnitude });
    });
}

#[op2]
#[serde]
pub fn bsengine_get_angular_velocity(#[string] name: String) -> Option<Vec<f32>> {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map(|v| vec![v.x, v.y, v.z]))
}

#[op2(fast)]
pub fn bsengine_get_angular_velocity_x(#[string] name: String) -> f32 {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.x))
}

#[op2(fast)]
pub fn bsengine_get_angular_velocity_y(#[string] name: String) -> f32 {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.y))
}

#[op2(fast)]
pub fn bsengine_get_angular_velocity_z(#[string] name: String) -> f32 {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.z))
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity_x(#[string] name: String, vx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocityX { name, vx });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity_y(#[string] name: String, vy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocityY { name, vy });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity_z(#[string] name: String, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocityZ { name, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_angular_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddAngularVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_angular_impulse(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddAngularImpulse { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_torque(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddTorque { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_set_ccd_enabled(#[string] name: String, enabled: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCCDEnabled { name, enabled });
    });
}

#[op2(fast)]
pub fn bsengine_set_linear_damping(#[string] name: String, damping: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetLinearDamping { name, damping });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_damping(#[string] name: String, damping: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularDamping { name, damping });
    });
}

#[op2(fast)]
pub fn bsengine_get_mass(#[string] name: String) -> f32 {
    MASS_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_gravity_scale(#[string] name: String) -> f32 {
    GRAVITY_SCALE_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(1.0))
}

#[op2(fast)]
pub fn bsengine_is_kinematic(#[string] name: String) -> bool {
    BODY_TYPE_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(false))
}

#[op2(fast)]
pub fn bsengine_is_sleeping(#[string] name: String) -> bool {
    SLEEP_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(false))
}

#[op2(fast)]
pub fn bsengine_wake_up(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::WakeUp { name }));
}

#[op2(fast)]
pub fn bsengine_sleep(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::PutToSleep { name }));
}

#[op2(fast)]
pub fn bsengine_is_collider_sensor(#[string] name: String) -> bool {
    COLLIDER_SENSOR_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(false))
}

#[op2(fast)]
pub fn bsengine_get_linear_damping(#[string] name: String) -> f32 {
    LINEAR_DAMPING_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_angular_damping(#[string] name: String) -> f32 {
    ANGULAR_DAMPING_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_restitution(#[string] name: String) -> f32 {
    RESTITUTION_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_set_restitution(#[string] name: String, restitution: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRestitution { name, restitution });
    });
}

#[op2(fast)]
pub fn bsengine_get_friction(#[string] name: String) -> f32 {
    FRICTION_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_set_friction(#[string] name: String, friction: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetFriction { name, friction });
    });
}

#[op2(fast)]
pub fn bsengine_set_mass(#[string] name: String, mass: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetMass { name, mass });
    });
}

#[op2(fast)]
pub fn bsengine_set_kinematic(#[string] name: String, kinematic: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetKinematic { name, kinematic });
    });
}

#[op2(fast)]
pub fn bsengine_set_gravity_scale(#[string] name: String, scale: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetGravityScale { name, scale });
    });
}

#[op2(fast)]
pub fn bsengine_set_collider_sensor(#[string] name: String, sensor: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetColliderSensor { name, sensor });
    });
}

#[op2(fast)]
pub fn bsengine_lock_rotation(#[string] name: String, lock_x: bool, lock_y: bool, lock_z: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::LockRotation {
            name,
            lock_x,
            lock_y,
            lock_z,
        });
    });
}

#[op2(fast)]
pub fn bsengine_lock_translation(#[string] name: String, lock_x: bool, lock_y: bool, lock_z: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::LockTranslation {
            name,
            lock_x,
            lock_y,
            lock_z,
        });
    });
}

#[op2(fast)]
pub fn bsengine_set_cursor_visible(visible: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCursorVisible { visible });
    });
}

#[op2(fast)]
pub fn bsengine_set_cursor_locked(locked: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCursorLocked { locked });
    });
}

#[op2(fast)]
pub fn bsengine_play_sound(#[string] path: String, volume: f32, loop_: bool) -> u32 {
    let id = SOUND_ID_COUNTER.with(|c| {
        let id = *c.borrow();
        *c.borrow_mut() = id + 1;
        id
    });
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::PlaySound {
            id,
            path,
            volume,
            loop_,
        });
    });
    id
}

#[op2(fast)]
pub fn bsengine_stop_sound(id: u32) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::StopSound { id }));
}

#[op2(fast)]
pub fn bsengine_pause_sound(id: u32) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::PauseSound { id }));
}

#[op2(fast)]
pub fn bsengine_resume_sound(id: u32) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ResumeSound { id }));
}

#[op2(fast)]
pub fn bsengine_set_sound_volume(id: u32, db: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSoundVolume { id, db })
    });
}

#[op2(fast)]
pub fn bsengine_set_sound_panning(id: u32, panning: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSoundPanning { id, panning })
    });
}

#[op2(fast)]
pub fn bsengine_set_sound_playback_rate(id: u32, rate: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetSoundPlaybackRate { id, rate })
    });
}

#[op2(fast)]
pub fn bsengine_seek_sound(id: u32, position: f64) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SeekSound { id, position })
    });
}

#[op2]
#[string]
pub fn bsengine_get_sound_state(id: u32) -> String {
    SOUND_STATE_SNAPSHOT
        .with(|s| s.borrow().get(&id).cloned())
        .unwrap_or_default()
}

#[op2(fast)]
pub fn bsengine_get_sound_position(id: u32) -> f64 {
    SOUND_POSITION_SNAPSHOT.with(|s| s.borrow().get(&id).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_set_hud_text(#[string] id: String, #[string] text: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::SetHudText { id, text }));
}

#[op2(fast)]
pub fn bsengine_clear_hud_text(#[string] id: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ClearHudText { id }));
}

// --- UI widget ops ---

#[op2(fast)]
pub fn bsengine_ui_set_label(
    #[string] id: String,
    #[string] text: String,
    x: f32,
    y: f32,
    font_size: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetUiLabel {
            id,
            text,
            x,
            y,
            font_size,
        })
    });
}

#[op2(fast)]
pub fn bsengine_ui_set_button(
    #[string] id: String,
    #[string] label: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetUiButton {
            id,
            label,
            x,
            y,
            width,
            height,
        })
    });
}

#[op2(fast)]
pub fn bsengine_ui_set_panel(
    #[string] id: String,
    #[string] title: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetUiPanel {
            id,
            title,
            x,
            y,
            width,
            height,
        })
    });
}

#[op2(fast)]
pub fn bsengine_ui_set_text_input(
    #[string] id: String,
    #[string] hint: String,
    x: f32,
    y: f32,
    width: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetUiTextInput {
            id,
            hint,
            x,
            y,
            width,
        })
    });
}

#[op2(fast)]
pub fn bsengine_ui_remove_widget(#[string] id: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::RemoveUiWidget { id }));
}

#[op2(fast)]
pub fn bsengine_ui_clear() {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ClearUiWidgets));
}

#[op2(fast)]
pub fn bsengine_ui_is_clicked(#[string] id: String) -> bool {
    UI_CLICKED_SNAPSHOT.with(|s| s.borrow().iter().any(|v| v == &id))
}

#[op2(fast)]
pub fn bsengine_load_scene(#[string] path: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::LoadScene { path }));
}

#[op2(fast)]
pub fn bsengine_set_skybox(#[string] path: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::SetSkybox { path }));
}

// --- Mouse ops ---

#[op2(fast)]
pub fn bsengine_is_mouse_pressed(button: u32) -> bool {
    if button > 7 {
        return false;
    }
    MOUSE_PRESSED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_mouse_down(button: u32) -> bool {
    if button > 7 {
        return false;
    }
    MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_mouse_up(button: u32) -> bool {
    if button > 7 {
        return false;
    }
    MOUSE_JUST_RELEASED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2]
#[serde]
pub fn bsengine_get_mouse_pos() -> Vec<f64> {
    MOUSE_POS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.0, v.1]
    })
}

#[op2]
#[serde]
pub fn bsengine_get_mouse_delta() -> Vec<f64> {
    MOUSE_DELTA_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.0, v.1]
    })
}

// --- Raycast op ---

#[op2]
#[serde]
pub fn bsengine_raycast(
    ox: f32,
    oy: f32,
    oz: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    max_dist: f32,
) -> Option<RaycastHitJson> {
    PHYSICS_WORLD_PTR.with(|p| {
        let ptr = *p.borrow();
        if ptr.is_null() {
            return None;
        }
        // SAFETY: ptr is valid for the duration of V8 execution (see plugin.rs run_scripts).
        let pw = unsafe { &*ptr };
        let dir_raw = Vec3::new(dx, dy, dz);
        let len = dir_raw.length();
        if len < 1e-7 {
            return None;
        }
        let origin = Vec3::new(ox, oy, oz);
        let dir = dir_raw / len;
        pw.cast_ray(origin, dir, max_dist).map(|hit| {
            let entity_name = hit
                .entity
                .and_then(|e| ENTITY_NAME_MAP.with(|m| m.borrow().get(&e.to_bits()).cloned()));
            RaycastHitJson {
                entity_name,
                point: [hit.point.x, hit.point.y, hit.point.z],
                normal: [hit.normal.x, hit.normal.y, hit.normal.z],
                distance: hit.distance,
            }
        })
    })
}

#[op2(fast)]
pub fn bsengine_is_gamepad_button(button: u32) -> bool {
    if button > 15 {
        return false;
    }
    GAMEPAD_BUTTON_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_gamepad_button_down(button: u32) -> bool {
    if button > 15 {
        return false;
    }
    GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_gamepad_button_up(button: u32) -> bool {
    if button > 15 {
        return false;
    }
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2]
#[serde]
pub fn bsengine_get_left_stick() -> Vec<f32> {
    GAMEPAD_STICKS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.0, v.1]
    })
}

#[op2]
#[serde]
pub fn bsengine_get_right_stick() -> Vec<f32> {
    GAMEPAD_STICKS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.2, v.3]
    })
}

#[op2(fast)]
pub fn bsengine_get_gamepad_trigger(side: u32) -> f32 {
    GAMEPAD_STICKS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        if side == 0 {
            v.4
        } else {
            v.5
        }
    })
}

deno_core::extension!(
    bsengine_ops,
    ops = [
        bsengine_log,
        bsengine_version,
        bsengine_get_transform,
        bsengine_get_forward_vector,
        bsengine_get_right_vector,
        bsengine_get_up_vector,
        bsengine_distance_to,
        bsengine_distance_to_point,
        bsengine_get_world_transform,
        bsengine_set_transform,
        bsengine_set_rotation,
        bsengine_set_rotation_euler,
        bsengine_set_scale,
        bsengine_add_position,
        bsengine_add_position_local,
        bsengine_set_position_x,
        bsengine_set_position_y,
        bsengine_set_position_z,
        bsengine_add_position_x,
        bsengine_add_position_y,
        bsengine_add_position_z,
        bsengine_rotate_by,
        bsengine_rotate_around_axis,
        bsengine_add_rotation_euler,
        bsengine_add_rotation_euler_x,
        bsengine_add_rotation_euler_y,
        bsengine_add_rotation_euler_z,
        bsengine_set_scale_x,
        bsengine_set_scale_y,
        bsengine_set_scale_z,
        bsengine_add_scale_x,
        bsengine_add_scale_y,
        bsengine_add_scale_z,
        bsengine_get_position_x,
        bsengine_get_position_y,
        bsengine_get_position_z,
        bsengine_get_scale_x,
        bsengine_get_scale_y,
        bsengine_get_scale_z,
        bsengine_get_rotation_euler_x,
        bsengine_get_rotation_euler_y,
        bsengine_get_rotation_euler_z,
        bsengine_add_scale,
        bsengine_set_rotation_euler_x,
        bsengine_set_rotation_euler_y,
        bsengine_set_rotation_euler_z,
        bsengine_multiply_scale,
        bsengine_is_key_pressed,
        bsengine_is_key_down,
        bsengine_is_key_up,
        bsengine_get_entity_names,
        bsengine_entity_exists,
        bsengine_get_entity_count,
        bsengine_get_entities_in_radius,
        bsengine_get_closest_entity,
        bsengine_set_kinematic,
        bsengine_set_gravity_scale,
        bsengine_set_collider_sensor,
        bsengine_set_emissive,
        bsengine_set_color,
        bsengine_spawn,
        bsengine_destroy,
        bsengine_set_visible,
        bsengine_get_visible,
        bsengine_get_material_color,
        bsengine_get_material_emissive,
        bsengine_set_metallic,
        bsengine_get_metallic,
        bsengine_set_roughness,
        bsengine_get_roughness,
        bsengine_material_set_shader,
        bsengine_material_clear_shader,
        bsengine_set_point_light_color,
        bsengine_set_point_light_intensity,
        bsengine_set_point_light_range,
        bsengine_set_spot_light_color,
        bsengine_set_spot_light_intensity,
        bsengine_set_spot_light_range,
        bsengine_set_spot_light_inner_angle,
        bsengine_set_spot_light_outer_angle,
        bsengine_set_directional_light_color,
        bsengine_set_directional_light_ambient,
        bsengine_set_directional_light_direction,
        bsengine_set_camera_fov,
        bsengine_set_camera_near,
        bsengine_set_camera_far,
        bsengine_set_damping,
        bsengine_play_animation,
        bsengine_pause_animation,
        bsengine_resume_animation,
        bsengine_reset_animation,
        bsengine_set_animation_speed,
        bsengine_set_animation_looping,
        bsengine_get_animation_clip,
        bsengine_get_animation_time,
        bsengine_get_animation_speed,
        bsengine_is_animation_playing,
        bsengine_is_animation_looping,
        bsengine_anim_set_trigger,
        bsengine_anim_set_float,
        bsengine_anim_set_bool,
        bsengine_anim_get_state,
        bsengine_set_lifetime,
        bsengine_get_lifetime,
        bsengine_damage_shield,
        bsengine_restore_shield,
        bsengine_set_max_shield,
        bsengine_get_shield,
        bsengine_get_max_shield,
        bsengine_get_shield_fraction,
        bsengine_is_shield_depleted,
        bsengine_reset_timer,
        bsengine_get_timer_elapsed,
        bsengine_get_timer_duration,
        bsengine_get_timer_fraction,
        bsengine_is_timer_finished,
        bsengine_is_timer_just_finished,
        bsengine_set_tween_duration,
        bsengine_set_tween_easing,
        bsengine_set_tween_repeat,
        bsengine_set_tween_elapsed,
        bsengine_get_tween_target_type,
        bsengine_get_tween_duration,
        bsengine_get_tween_easing,
        bsengine_get_tween_repeat,
        bsengine_get_tween_elapsed,
        bsengine_get_tween_progress,
        bsengine_is_tween_finished,
        bsengine_is_tween_reversed,
        bsengine_set_follow_target,
        bsengine_set_follow_offset,
        bsengine_set_follow_speed,
        bsengine_get_follow_target,
        bsengine_get_follow_offset_x,
        bsengine_get_follow_offset_y,
        bsengine_get_follow_offset_z,
        bsengine_get_follow_speed,
        bsengine_set_look_at_target,
        bsengine_set_look_at_up,
        bsengine_get_look_at_target,
        bsengine_get_look_at_up_x,
        bsengine_get_look_at_up_y,
        bsengine_get_look_at_up_z,
        bsengine_get_network_id,
        bsengine_get_network_authority,
        bsengine_get_network_peer_id,
        bsengine_is_network_replicated,
        bsengine_network_start_server,
        bsengine_network_connect,
        bsengine_network_disconnect,
        bsengine_network_is_server,
        bsengine_network_is_connected,
        bsengine_network_get_my_peer_id,
        bsengine_network_get_peer_count,
        bsengine_look_at,
        bsengine_get_time,
        bsengine_get_delta_time,
        bsengine_get_screen_size,
        bsengine_set_parent,
        bsengine_clear_parent,
        bsengine_get_parent,
        bsengine_get_children,
        bsengine_get_velocity,
        bsengine_get_linear_speed,
        bsengine_get_velocity_x,
        bsengine_get_velocity_y,
        bsengine_get_velocity_z,
        bsengine_add_impulse,
        bsengine_apply_impulse_at_point,
        bsengine_add_force,
        bsengine_add_force_at_point,
        bsengine_reset_forces,
        bsengine_set_velocity,
        bsengine_set_velocity_x,
        bsengine_set_velocity_y,
        bsengine_set_velocity_z,
        bsengine_get_gravity,
        bsengine_set_gravity,
        bsengine_get_angular_velocity,
        bsengine_get_angular_velocity_x,
        bsengine_get_angular_velocity_y,
        bsengine_get_angular_velocity_z,
        bsengine_set_angular_velocity,
        bsengine_set_angular_velocity_x,
        bsengine_set_angular_velocity_y,
        bsengine_set_angular_velocity_z,
        bsengine_add_velocity,
        bsengine_add_angular_velocity,
        bsengine_add_angular_impulse,
        bsengine_add_torque,
        bsengine_set_ccd_enabled,
        bsengine_set_linear_damping,
        bsengine_set_angular_damping,
        bsengine_get_mass,
        bsengine_set_mass,
        bsengine_get_gravity_scale,
        bsengine_is_kinematic,
        bsengine_is_sleeping,
        bsengine_wake_up,
        bsengine_sleep,
        bsengine_is_collider_sensor,
        bsengine_get_linear_damping,
        bsengine_get_angular_damping,
        bsengine_get_restitution,
        bsengine_set_restitution,
        bsengine_get_friction,
        bsengine_set_friction,
        bsengine_lock_rotation,
        bsengine_lock_translation,
        bsengine_set_cursor_visible,
        bsengine_set_cursor_locked,
        bsengine_play_sound,
        bsengine_stop_sound,
        bsengine_pause_sound,
        bsengine_resume_sound,
        bsengine_set_sound_volume,
        bsengine_set_sound_panning,
        bsengine_set_sound_playback_rate,
        bsengine_seek_sound,
        bsengine_get_sound_state,
        bsengine_get_sound_position,
        bsengine_set_hud_text,
        bsengine_clear_hud_text,
        bsengine_ui_set_label,
        bsengine_ui_set_button,
        bsengine_ui_set_panel,
        bsengine_ui_set_text_input,
        bsengine_ui_remove_widget,
        bsengine_ui_clear,
        bsengine_ui_is_clicked,
        bsengine_load_scene,
        bsengine_is_mouse_pressed,
        bsengine_is_mouse_down,
        bsengine_is_mouse_up,
        bsengine_get_mouse_pos,
        bsengine_get_mouse_delta,
        bsengine_raycast,
        bsengine_is_gamepad_button,
        bsengine_is_gamepad_button_down,
        bsengine_is_gamepad_button_up,
        bsengine_get_left_stick,
        bsengine_get_right_stick,
        bsengine_get_gamepad_trigger,
        bsengine_set_skybox,
        bsengine_set_nav_destination,
        bsengine_clear_nav_destination,
        bsengine_set_nav_speed,
        bsengine_set_nav_angular_speed,
        bsengine_set_nav_stopping_distance,
        bsengine_set_nav_enabled,
        bsengine_get_nav_speed,
        bsengine_get_nav_angular_speed,
        bsengine_get_nav_stopping_distance,
        bsengine_is_nav_moving,
        bsengine_has_nav_arrived,
        bsengine_is_nav_idle,
        bsengine_nav_has_no_path,
        bsengine_is_nav_enabled,
        bsengine_navmesh_init,
        bsengine_navmesh_set_walkable,
        bsengine_navmesh_get_state,
        bsengine_save_game,
        bsengine_load_game,
        bsengine_set_bloom_intensity,
        bsengine_set_bloom_threshold,
        bsengine_set_bloom_radius,
        bsengine_set_bloom_softness,
        bsengine_set_bloom_enabled,
        bsengine_get_bloom_intensity,
        bsengine_get_bloom_threshold,
        bsengine_get_bloom_radius,
        bsengine_get_bloom_softness,
        bsengine_is_bloom_enabled,
        bsengine_set_ao_radius,
        bsengine_set_ao_bias,
        bsengine_set_ao_intensity,
        bsengine_set_ao_sample_count,
        bsengine_set_ao_enabled,
        bsengine_get_ao_radius,
        bsengine_get_ao_bias,
        bsengine_get_ao_intensity,
        bsengine_get_ao_sample_count,
        bsengine_is_ao_enabled,
        bsengine_set_tone_map_mode,
        bsengine_set_tone_map_exposure,
        bsengine_set_tone_map_enabled,
        bsengine_get_tone_map_mode,
        bsengine_get_tone_map_exposure,
        bsengine_is_tone_map_enabled,
    ],
);

pub const BOOTSTRAP_JS: &str = r#"
// `var`, not `const`: scene reload re-runs this bootstrap in the SAME V8
// isolate/global scope (see handle_scene_load in bsengine-runtime) rather
// than spinning up a new isolate. `const`/`let` at top level would throw
// "Identifier 'Bsengine' has already been declared" on the second run;
// `var` (and plain reassignment) is redeclaration-safe.
var Bsengine = {
    log:            (msg)                  => Deno.core.ops.bsengine_log(msg),
    version:        ()                     => Deno.core.ops.bsengine_version(),
    getTransform:      (name)                 => Deno.core.ops.bsengine_get_transform(name),
    getPosition:       (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? { x: t.x, y: t.y, z: t.z } : null; },
    getRotation:       (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? { x: t.rx, y: t.ry, z: t.rz, w: t.rw } : null; },
    getScale:          (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? { x: t.sx, y: t.sy, z: t.sz } : null; },
    getForwardVector:  (name)                 => Deno.core.ops.bsengine_get_forward_vector(name),
    getRightVector:    (name)                 => Deno.core.ops.bsengine_get_right_vector(name),
    getUpVector:       (name)                 => Deno.core.ops.bsengine_get_up_vector(name),
    distanceTo:        (nameA, nameB)         => Deno.core.ops.bsengine_distance_to(nameA, nameB),
    distanceToPoint:   (name, x, y, z)       => Deno.core.ops.bsengine_distance_to_point(name, x, y, z),
    getWorldTransform: (name)                 => Deno.core.ops.bsengine_get_world_transform(name),
    getWorldPosition:  (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? { x: t.x, y: t.y, z: t.z } : null; },
    getWorldRotation:  (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? { x: t.rx, y: t.ry, z: t.rz, w: t.rw } : null; },
    getWorldScale:     (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? { x: t.sx, y: t.sy, z: t.sz } : null; },
    setTransform:   (name, x, y, z)        => Deno.core.ops.bsengine_set_transform(name, x, y, z),
    setRotation:      (name, rx, ry, rz, rw)        => Deno.core.ops.bsengine_set_rotation(name, rx, ry, rz, rw),
    setRotationEuler: (name, pitch, yaw, roll)      => Deno.core.ops.bsengine_set_rotation_euler(name, pitch, yaw, roll),
    setScale:            (name, sx, sy, sz)     => Deno.core.ops.bsengine_set_scale(name, sx, sy, sz),
    addPosition:         (name, dx, dy, dz)     => Deno.core.ops.bsengine_add_position(name, dx, dy, dz),
    addPositionLocal:    (name, dx, dy, dz)     => Deno.core.ops.bsengine_add_position_local(name, dx, dy, dz),
    setPositionX:        (name, x)              => Deno.core.ops.bsengine_set_position_x(name, x),
    setPositionY:        (name, y)              => Deno.core.ops.bsengine_set_position_y(name, y),
    setPositionZ:        (name, z)              => Deno.core.ops.bsengine_set_position_z(name, z),
    addPositionX:        (name, dx)             => Deno.core.ops.bsengine_add_position_x(name, dx),
    addPositionY:        (name, dy)             => Deno.core.ops.bsengine_add_position_y(name, dy),
    addPositionZ:        (name, dz)             => Deno.core.ops.bsengine_add_position_z(name, dz),
    rotateBy:          (name, rx, ry, rz, rw)   => Deno.core.ops.bsengine_rotate_by(name, rx, ry, rz, rw),
    rotateAroundAxis:  (name, ax, ay, az, deg)  => Deno.core.ops.bsengine_rotate_around_axis(name, ax, ay, az, deg),
    addRotationEuler:  (name, pitch, yaw, roll) => Deno.core.ops.bsengine_add_rotation_euler(name, pitch, yaw, roll),
    addRotationEulerX: (name, deg) => Deno.core.ops.bsengine_add_rotation_euler_x(name, deg),
    addRotationEulerY: (name, deg) => Deno.core.ops.bsengine_add_rotation_euler_y(name, deg),
    addRotationEulerZ: (name, deg) => Deno.core.ops.bsengine_add_rotation_euler_z(name, deg),
    setScaleX:         (name, x)               => Deno.core.ops.bsengine_set_scale_x(name, x),
    setScaleY:         (name, y)               => Deno.core.ops.bsengine_set_scale_y(name, y),
    setScaleZ:         (name, z)               => Deno.core.ops.bsengine_set_scale_z(name, z),
    addScaleX:         (name, dx)              => Deno.core.ops.bsengine_add_scale_x(name, dx),
    addScaleY:         (name, dy)              => Deno.core.ops.bsengine_add_scale_y(name, dy),
    addScaleZ:         (name, dz)              => Deno.core.ops.bsengine_add_scale_z(name, dz),
    getPositionX:      (name)                 => Deno.core.ops.bsengine_get_position_x(name),
    getPositionY:      (name)                 => Deno.core.ops.bsengine_get_position_y(name),
    getPositionZ:      (name)                 => Deno.core.ops.bsengine_get_position_z(name),
    getScaleX:         (name)                 => Deno.core.ops.bsengine_get_scale_x(name),
    getScaleY:         (name)                 => Deno.core.ops.bsengine_get_scale_y(name),
    getScaleZ:         (name)                 => Deno.core.ops.bsengine_get_scale_z(name),
    getRotationEulerX: (name) => Deno.core.ops.bsengine_get_rotation_euler_x(name),
    getRotationEulerY: (name) => Deno.core.ops.bsengine_get_rotation_euler_y(name),
    getRotationEulerZ: (name) => Deno.core.ops.bsengine_get_rotation_euler_z(name),
    addScale:          (name, sx, sy, sz)       => Deno.core.ops.bsengine_add_scale(name, sx, sy, sz),
    setRotationEulerX: (name, deg) => Deno.core.ops.bsengine_set_rotation_euler_x(name, deg),
    setRotationEulerY: (name, deg) => Deno.core.ops.bsengine_set_rotation_euler_y(name, deg),
    setRotationEulerZ: (name, deg) => Deno.core.ops.bsengine_set_rotation_euler_z(name, deg),
    multiplyScale:     (name, sx, sy, sz) => Deno.core.ops.bsengine_multiply_scale(name, sx, sy, sz),
    isKeyPressed:   (key)                  => Deno.core.ops.bsengine_is_key_pressed(key),
    isKeyDown:      (key)                  => Deno.core.ops.bsengine_is_key_down(key),
    isKeyUp:        (key)                  => Deno.core.ops.bsengine_is_key_up(key),
    getEntityNames:      ()    => JSON.parse(Deno.core.ops.bsengine_get_entity_names()),
    entityExists:        (name) => Deno.core.ops.bsengine_entity_exists(name),
    getEntityCount:      ()    => Deno.core.ops.bsengine_get_entity_count(),
    getEntitiesInRadius:     (x, y, z, radius) => JSON.parse(Deno.core.ops.bsengine_get_entities_in_radius(x, y, z, radius)),
    getClosestEntity:        (x, y, z)       => Deno.core.ops.bsengine_get_closest_entity(x, y, z),
    setKinematic:        (name, kinematic) => Deno.core.ops.bsengine_set_kinematic(name, kinematic),
    setGravityScale:     (name, scale)     => Deno.core.ops.bsengine_set_gravity_scale(name, scale),
    setColliderSensor:   (name, sensor)    => Deno.core.ops.bsengine_set_collider_sensor(name, sensor),
    setEmissive:    (name, r, g, b)        => Deno.core.ops.bsengine_set_emissive(name, r, g, b),
    setColor:       (name, r, g, b)        => Deno.core.ops.bsengine_set_color(name, r, g, b),
    spawn:          (params)               => Deno.core.ops.bsengine_spawn(params),
    destroy:        (name)                 => Deno.core.ops.bsengine_destroy(name),
    setVisible:     (name, v)              => Deno.core.ops.bsengine_set_visible(name, v),
    getVisible:     (name)                 => Deno.core.ops.bsengine_get_visible(name),
    getMaterialColor:   (name) => { const v = Deno.core.ops.bsengine_get_material_color(name); return v ? { r: v[0], g: v[1], b: v[2] } : null; },
    getMaterialEmissive:(name) => { const v = Deno.core.ops.bsengine_get_material_emissive(name); return v ? { r: v[0], g: v[1], b: v[2] } : null; },
    setMetallic:    (name, value)          => Deno.core.ops.bsengine_set_metallic(name, value),
    getMetallic:    (name)                 => Deno.core.ops.bsengine_get_metallic(name),
    setRoughness:           (name, value)       => Deno.core.ops.bsengine_set_roughness(name, value),
    getRoughness:           (name)              => Deno.core.ops.bsengine_get_roughness(name),
    setShader:              (name, path)        => Deno.core.ops.bsengine_material_set_shader(name, path),
    clearShader:            (name)              => Deno.core.ops.bsengine_material_clear_shader(name),
    setPointLightColor:     (name, r, g, b)     => Deno.core.ops.bsengine_set_point_light_color(name, r, g, b),
    setPointLightIntensity: (name, value)       => Deno.core.ops.bsengine_set_point_light_intensity(name, value),
    setPointLightRange:     (name, value)       => Deno.core.ops.bsengine_set_point_light_range(name, value),
    setSpotLightColor:      (name, r, g, b)     => Deno.core.ops.bsengine_set_spot_light_color(name, r, g, b),
    setSpotLightIntensity:  (name, value)       => Deno.core.ops.bsengine_set_spot_light_intensity(name, value),
    setSpotLightRange:      (name, value)       => Deno.core.ops.bsengine_set_spot_light_range(name, value),
    setSpotLightInnerAngle: (name, deg)         => Deno.core.ops.bsengine_set_spot_light_inner_angle(name, deg),
    setSpotLightOuterAngle: (name, deg)         => Deno.core.ops.bsengine_set_spot_light_outer_angle(name, deg),
    setDirectionalLightColor:     (name, r, g, b) => Deno.core.ops.bsengine_set_directional_light_color(name, r, g, b),
    setDirectionalLightAmbient:   (name, r, g, b) => Deno.core.ops.bsengine_set_directional_light_ambient(name, r, g, b),
    setDirectionalLightDirection: (name, x, y, z) => Deno.core.ops.bsengine_set_directional_light_direction(name, x, y, z),
    setCameraFov:   (name, deg)            => Deno.core.ops.bsengine_set_camera_fov(name, deg),
    setCameraNear:  (name, value)          => Deno.core.ops.bsengine_set_camera_near(name, value),
    setCameraFar:   (name, value)          => Deno.core.ops.bsengine_set_camera_far(name, value),
    setDamping:         (name, value)      => Deno.core.ops.bsengine_set_damping(name, value),
    playAnimation:          (name, clip)    => Deno.core.ops.bsengine_play_animation(name, clip),
    pauseAnimation:         (name)          => Deno.core.ops.bsengine_pause_animation(name),
    resumeAnimation:        (name)          => Deno.core.ops.bsengine_resume_animation(name),
    resetAnimation:         (name)          => Deno.core.ops.bsengine_reset_animation(name),
    setAnimationSpeed:      (name, speed)   => Deno.core.ops.bsengine_set_animation_speed(name, speed),
    setAnimationLooping:    (name, looping) => Deno.core.ops.bsengine_set_animation_looping(name, looping),
    getAnimationClip:       (name)          => Deno.core.ops.bsengine_get_animation_clip(name),
    getAnimationTime:       (name)          => Deno.core.ops.bsengine_get_animation_time(name),
    getAnimationSpeed:      (name)          => Deno.core.ops.bsengine_get_animation_speed(name),
    isAnimationPlaying:     (name)          => Deno.core.ops.bsengine_is_animation_playing(name),
    isAnimationLooping:     (name)          => Deno.core.ops.bsengine_is_animation_looping(name),
    asmSetTrigger:          (name, trigger)      => Deno.core.ops.bsengine_anim_set_trigger(name, trigger),
    asmSetFloat:            (name, param, value) => Deno.core.ops.bsengine_anim_set_float(name, param, value),
    asmSetBool:             (name, param, value) => Deno.core.ops.bsengine_anim_set_bool(name, param, !!value),
    asmGetState:            (name)               => Deno.core.ops.bsengine_anim_get_state(name),
    setLifetime:            (name, seconds) => Deno.core.ops.bsengine_set_lifetime(name, seconds),
    getLifetime:            (name)          => Deno.core.ops.bsengine_get_lifetime(name),
    damageShield:           (name, amount)  => Deno.core.ops.bsengine_damage_shield(name, amount),
    restoreShield:          (name, amount)  => Deno.core.ops.bsengine_restore_shield(name, amount),
    setMaxShield:           (name, value)   => Deno.core.ops.bsengine_set_max_shield(name, value),
    getShield:              (name)          => Deno.core.ops.bsengine_get_shield(name),
    getMaxShield:           (name)          => Deno.core.ops.bsengine_get_max_shield(name),
    getShieldFraction:      (name)          => Deno.core.ops.bsengine_get_shield_fraction(name),
    isShieldDepleted:       (name)          => Deno.core.ops.bsengine_is_shield_depleted(name),
    resetTimer:             (name)          => Deno.core.ops.bsengine_reset_timer(name),
    getTimerElapsed:        (name)          => Deno.core.ops.bsengine_get_timer_elapsed(name),
    getTimerDuration:       (name)          => Deno.core.ops.bsengine_get_timer_duration(name),
    getTimerFraction:       (name)          => Deno.core.ops.bsengine_get_timer_fraction(name),
    isTimerFinished:        (name)          => Deno.core.ops.bsengine_is_timer_finished(name),
    isTimerJustFinished:    (name)          => Deno.core.ops.bsengine_is_timer_just_finished(name),
    setNavDestination:      (name, x, y, z) => Deno.core.ops.bsengine_set_nav_destination(name, x, y, z),
    clearNavDestination:    (name)          => Deno.core.ops.bsengine_clear_nav_destination(name),
    setNavSpeed:            (name, speed)   => Deno.core.ops.bsengine_set_nav_speed(name, speed),
    setNavAngularSpeed:     (name, speed)   => Deno.core.ops.bsengine_set_nav_angular_speed(name, speed),
    setNavStoppingDistance: (name, dist)    => Deno.core.ops.bsengine_set_nav_stopping_distance(name, dist),
    setNavEnabled:          (name, enabled) => Deno.core.ops.bsengine_set_nav_enabled(name, enabled),
    getNavSpeed:            (name)          => Deno.core.ops.bsengine_get_nav_speed(name),
    getNavAngularSpeed:     (name)          => Deno.core.ops.bsengine_get_nav_angular_speed(name),
    getNavStoppingDistance: (name)          => Deno.core.ops.bsengine_get_nav_stopping_distance(name),
    isNavMoving:            (name)          => Deno.core.ops.bsengine_is_nav_moving(name),
    hasNavArrived:          (name)          => Deno.core.ops.bsengine_has_nav_arrived(name),
    isNavIdle:              (name)          => Deno.core.ops.bsengine_is_nav_idle(name),
    navHasNoPath:           (name)          => Deno.core.ops.bsengine_nav_has_no_path(name),
    isNavEnabled:           (name)          => Deno.core.ops.bsengine_is_nav_enabled(name),
    setBloomIntensity:  (name, v)          => Deno.core.ops.bsengine_set_bloom_intensity(name, v),
    setBloomThreshold:  (name, v)          => Deno.core.ops.bsengine_set_bloom_threshold(name, v),
    setBloomRadius:     (name, v)          => Deno.core.ops.bsengine_set_bloom_radius(name, v),
    setBloomSoftness:   (name, v)          => Deno.core.ops.bsengine_set_bloom_softness(name, v),
    setBloomEnabled:    (name, v)          => Deno.core.ops.bsengine_set_bloom_enabled(name, v),
    getBloomIntensity:  (name)             => Deno.core.ops.bsengine_get_bloom_intensity(name),
    getBloomThreshold:  (name)             => Deno.core.ops.bsengine_get_bloom_threshold(name),
    getBloomRadius:     (name)             => Deno.core.ops.bsengine_get_bloom_radius(name),
    getBloomSoftness:   (name)             => Deno.core.ops.bsengine_get_bloom_softness(name),
    isBloomEnabled:     (name)             => Deno.core.ops.bsengine_is_bloom_enabled(name),
    setAoRadius:        (name, v)          => Deno.core.ops.bsengine_set_ao_radius(name, v),
    setAoBias:          (name, v)          => Deno.core.ops.bsengine_set_ao_bias(name, v),
    setAoIntensity:     (name, v)          => Deno.core.ops.bsengine_set_ao_intensity(name, v),
    setAoSampleCount:   (name, v)          => Deno.core.ops.bsengine_set_ao_sample_count(name, v),
    setAoEnabled:       (name, v)          => Deno.core.ops.bsengine_set_ao_enabled(name, v),
    getAoRadius:        (name)             => Deno.core.ops.bsengine_get_ao_radius(name),
    getAoBias:          (name)             => Deno.core.ops.bsengine_get_ao_bias(name),
    getAoIntensity:     (name)             => Deno.core.ops.bsengine_get_ao_intensity(name),
    getAoSampleCount:   (name)             => Deno.core.ops.bsengine_get_ao_sample_count(name),
    isAoEnabled:        (name)             => Deno.core.ops.bsengine_is_ao_enabled(name),
    setToneMapMode:     (name, v)          => Deno.core.ops.bsengine_set_tone_map_mode(name, v),
    setToneMapExposure: (name, v)          => Deno.core.ops.bsengine_set_tone_map_exposure(name, v),
    setToneMapEnabled:  (name, v)          => Deno.core.ops.bsengine_set_tone_map_enabled(name, v),
    getToneMapMode:     (name)             => Deno.core.ops.bsengine_get_tone_map_mode(name),
    getToneMapExposure: (name)             => Deno.core.ops.bsengine_get_tone_map_exposure(name),
    isToneMapEnabled:   (name)             => Deno.core.ops.bsengine_is_tone_map_enabled(name),
    setTweenDuration:(name, duration)      => Deno.core.ops.bsengine_set_tween_duration(name, duration),
    setTweenEasing: (name, easing)         => Deno.core.ops.bsengine_set_tween_easing(name, easing),
    setTweenRepeat: (name, repeat)         => Deno.core.ops.bsengine_set_tween_repeat(name, repeat),
    setTweenElapsed:(name, elapsed)        => Deno.core.ops.bsengine_set_tween_elapsed(name, elapsed),
    getTweenTargetType:(name)              => Deno.core.ops.bsengine_get_tween_target_type(name),
    getTweenDuration:(name)                => Deno.core.ops.bsengine_get_tween_duration(name),
    getTweenEasing: (name)                 => Deno.core.ops.bsengine_get_tween_easing(name),
    getTweenRepeat: (name)                 => Deno.core.ops.bsengine_get_tween_repeat(name),
    getTweenElapsed:(name)                 => Deno.core.ops.bsengine_get_tween_elapsed(name),
    getTweenProgress:(name)                => Deno.core.ops.bsengine_get_tween_progress(name),
    isTweenFinished:(name)                 => Deno.core.ops.bsengine_is_tween_finished(name),
    isTweenReversed:(name)                 => Deno.core.ops.bsengine_is_tween_reversed(name),
    setFollowTarget:(name, target)         => Deno.core.ops.bsengine_set_follow_target(name, target),
    setFollowOffset:(name, x, y, z)        => Deno.core.ops.bsengine_set_follow_offset(name, x, y, z),
    setFollowSpeed: (name, speed)          => Deno.core.ops.bsengine_set_follow_speed(name, speed),
    getFollowTarget:(name)                 => JSON.parse(Deno.core.ops.bsengine_get_follow_target(name)),
    getFollowOffsetX:(name)               => Deno.core.ops.bsengine_get_follow_offset_x(name),
    getFollowOffsetY:(name)               => Deno.core.ops.bsengine_get_follow_offset_y(name),
    getFollowOffsetZ:(name)               => Deno.core.ops.bsengine_get_follow_offset_z(name),
    getFollowSpeed: (name)                 => Deno.core.ops.bsengine_get_follow_speed(name),
    setLookAtTarget:(name, target)         => Deno.core.ops.bsengine_set_look_at_target(name, target),
    setLookAtUp:    (name, x, y, z)        => Deno.core.ops.bsengine_set_look_at_up(name, x, y, z),
    getLookAtTarget:(name)                 => JSON.parse(Deno.core.ops.bsengine_get_look_at_target(name)),
    getLookAtUpX:   (name)                 => Deno.core.ops.bsengine_get_look_at_up_x(name),
    getLookAtUpY:   (name)                 => Deno.core.ops.bsengine_get_look_at_up_y(name),
    getLookAtUpZ:   (name)                 => Deno.core.ops.bsengine_get_look_at_up_z(name),
    // Amplify
    // Barrier
    // Beacon
    // ShieldBreak
    // Root
    // Slow
    // Stun (severity: 0=Light, 1=Heavy, 2=Knockdown)

    // Invincible

    // Isolate

    // Jeer

    // Jetpack

    // Jolt

    // Jostle

    // Juke






























    // NetworkId
    getNetworkId:                  (name)           => Deno.core.ops.bsengine_get_network_id(name),
    getNetworkAuthority:           (name)           => Deno.core.ops.bsengine_get_network_authority(name),
    getNetworkPeerId:              (name)           => Deno.core.ops.bsengine_get_network_peer_id(name),
    isNetworkReplicated:           (name)           => Deno.core.ops.bsengine_is_network_replicated(name),

    // Network session
    network: {
      startServer:   (port)        => Deno.core.ops.bsengine_network_start_server(port),
      connect:       (host, port) => Deno.core.ops.bsengine_network_connect(host, port),
      disconnect:    ()           => Deno.core.ops.bsengine_network_disconnect(),
      isServer:      ()           => Deno.core.ops.bsengine_network_is_server(),
      isConnected:   ()           => Deno.core.ops.bsengine_network_is_connected(),
      getMyPeerId:   ()           => Deno.core.ops.bsengine_network_get_my_peer_id(),
      getPeerCount:  ()           => Deno.core.ops.bsengine_network_get_peer_count(),
    },

    // Nimble

    // Notice

    // Nourish

    // Nova

    // Npc

    // Nullify

    // Numb






































    lookAt:         (name, tx, ty, tz)     => Deno.core.ops.bsengine_look_at(name, tx, ty, tz),

    // Time
    getTime:        ()                     => Deno.core.ops.bsengine_get_time(),
    getDeltaTime:   ()                     => Deno.core.ops.bsengine_get_delta_time(),
    getScreenSize:  ()                     => { const [w, h] = Deno.core.ops.bsengine_get_screen_size(); return { width: w, height: h }; },
    setParent:      (child, parent)        => Deno.core.ops.bsengine_set_parent(child, parent),
    clearParent:      (child)   => Deno.core.ops.bsengine_clear_parent(child),
    getParent:        (name)    => { const r = Deno.core.ops.bsengine_get_parent(name); return JSON.parse(r); },
    getChildren:         (name)         => JSON.parse(Deno.core.ops.bsengine_get_children(name)),
    getChildrenCount:    (name)         => JSON.parse(Deno.core.ops.bsengine_get_children(name)).length,
    getChildAt:          (name, index)  => { const c = JSON.parse(Deno.core.ops.bsengine_get_children(name)); return c[index] ?? null; },
    getVelocity:      (name)    => { const v = Deno.core.ops.bsengine_get_velocity(name); return v ? { x: v[0], y: v[1], z: v[2] } : null; },
    getLinearSpeed:   (name)    => { const s = Deno.core.ops.bsengine_get_linear_speed(name); return s !== null && s !== undefined ? s[0] : -1; },
    getVelocityX:     (name) => Deno.core.ops.bsengine_get_velocity_x(name),
    getVelocityY:     (name) => Deno.core.ops.bsengine_get_velocity_y(name),
    getVelocityZ:     (name) => Deno.core.ops.bsengine_get_velocity_z(name),
    addImpulse:       (name, fx, fy, fz) => Deno.core.ops.bsengine_add_impulse(name, fx, fy, fz),
    applyImpulseAtPoint: (name, fx, fy, fz, px, py, pz) => Deno.core.ops.bsengine_apply_impulse_at_point(name, fx, fy, fz, px, py, pz),
    addForce:         (name, fx, fy, fz) => Deno.core.ops.bsengine_add_force(name, fx, fy, fz),
    addForceAtPoint:  (name, fx, fy, fz, px, py, pz) => Deno.core.ops.bsengine_add_force_at_point(name, fx, fy, fz, px, py, pz),
    // Clears any force/torque accumulated via addForce/addTorque — those
    // persist across steps until explicitly cleared (Rapier's documented
    // behavior), so stopping a body needs this alongside setVelocity(0,0,0)
    // or a held-over force reintroduces motion on the next physics step.
    resetForces:      (name) => Deno.core.ops.bsengine_reset_forces(name),
    setVelocity:      (name, vx, vy, vz) => Deno.core.ops.bsengine_set_velocity(name, vx, vy, vz),
    setVelocityX:     (name, vx) => Deno.core.ops.bsengine_set_velocity_x(name, vx),
    setVelocityY:     (name, vy) => Deno.core.ops.bsengine_set_velocity_y(name, vy),
    setVelocityZ:     (name, vz) => Deno.core.ops.bsengine_set_velocity_z(name, vz),
    getGravity:           ()                     => Deno.core.ops.bsengine_get_gravity(),
    setGravity:           (magnitude)             => Deno.core.ops.bsengine_set_gravity(magnitude),
    getAngularVelocity:   (name)                  => { const v = Deno.core.ops.bsengine_get_angular_velocity(name); return v ? { x: v[0], y: v[1], z: v[2] } : null; },
    getAngularVelocityX:  (name) => Deno.core.ops.bsengine_get_angular_velocity_x(name),
    getAngularVelocityY:  (name) => Deno.core.ops.bsengine_get_angular_velocity_y(name),
    getAngularVelocityZ:  (name) => Deno.core.ops.bsengine_get_angular_velocity_z(name),
    setAngularVelocity:   (name, vx, vy, vz)      => Deno.core.ops.bsengine_set_angular_velocity(name, vx, vy, vz),
    setAngularVelocityX:  (name, vx) => Deno.core.ops.bsengine_set_angular_velocity_x(name, vx),
    setAngularVelocityY:  (name, vy) => Deno.core.ops.bsengine_set_angular_velocity_y(name, vy),
    setAngularVelocityZ:  (name, vz) => Deno.core.ops.bsengine_set_angular_velocity_z(name, vz),
    addVelocity:          (name, vx, vy, vz) => Deno.core.ops.bsengine_add_velocity(name, vx, vy, vz),
    addAngularVelocity:   (name, vx, vy, vz) => Deno.core.ops.bsengine_add_angular_velocity(name, vx, vy, vz),
    addAngularImpulse:    (name, vx, vy, vz)      => Deno.core.ops.bsengine_add_angular_impulse(name, vx, vy, vz),
    addTorque:            (name, vx, vy, vz)      => Deno.core.ops.bsengine_add_torque(name, vx, vy, vz),
    setCCDEnabled:        (name, enabled)           => Deno.core.ops.bsengine_set_ccd_enabled(name, enabled),
    setLinearDamping:     (name, damping)          => Deno.core.ops.bsengine_set_linear_damping(name, damping),
    setAngularDamping:    (name, damping)          => Deno.core.ops.bsengine_set_angular_damping(name, damping),
    getMass:              (name)                   => Deno.core.ops.bsengine_get_mass(name),
    setMass:              (name, mass)             => Deno.core.ops.bsengine_set_mass(name, mass),
    getGravityScale:      (name)                   => Deno.core.ops.bsengine_get_gravity_scale(name),
    isKinematic:          (name)                   => Deno.core.ops.bsengine_is_kinematic(name),
    isSleeping:           (name)                   => Deno.core.ops.bsengine_is_sleeping(name),
    wakeUp:               (name)                   => Deno.core.ops.bsengine_wake_up(name),
    sleep:                (name)                   => Deno.core.ops.bsengine_sleep(name),
    isColliderSensor:     (name)                   => Deno.core.ops.bsengine_is_collider_sensor(name),
    getLinearDamping:     (name)                   => Deno.core.ops.bsengine_get_linear_damping(name),
    getAngularDamping:    (name)                   => Deno.core.ops.bsengine_get_angular_damping(name),
    getRestitution:       (name)                   => Deno.core.ops.bsengine_get_restitution(name),
    setRestitution:       (name, v)                => Deno.core.ops.bsengine_set_restitution(name, v),
    getFriction:          (name)                   => Deno.core.ops.bsengine_get_friction(name),
    setFriction:          (name, v)                => Deno.core.ops.bsengine_set_friction(name, v),
    lockRotation:         (name, lockX, lockY, lockZ) => Deno.core.ops.bsengine_lock_rotation(name, lockX, lockY, lockZ),
    lockTranslation:      (name, lockX, lockY, lockZ) => Deno.core.ops.bsengine_lock_translation(name, lockX, lockY, lockZ),
    setCursorVisible: (visible) => Deno.core.ops.bsengine_set_cursor_visible(visible),
    setCursorLocked:  (locked)  => Deno.core.ops.bsengine_set_cursor_locked(locked),
    playSound:      (path, opts) => {
        const v = (opts && opts.volume !== undefined) ? opts.volume : 1.0;
        const l = (opts && opts.loop) ? true : false;
        return Deno.core.ops.bsengine_play_sound(path, v, l);
    },
    stopSound:      (id)                   => Deno.core.ops.bsengine_stop_sound(id),
    pauseSound:     (id)                   => Deno.core.ops.bsengine_pause_sound(id),
    resumeSound:    (id)                   => Deno.core.ops.bsengine_resume_sound(id),
    setSoundVolume:       (id, db)      => Deno.core.ops.bsengine_set_sound_volume(id, db),
    setSoundPanning:      (id, panning) => Deno.core.ops.bsengine_set_sound_panning(id, panning),
    setSoundPlaybackRate: (id, rate)    => Deno.core.ops.bsengine_set_sound_playback_rate(id, rate),
    seekSound:            (id, pos)     => Deno.core.ops.bsengine_seek_sound(id, pos),
    getSoundState:        (id)          => Deno.core.ops.bsengine_get_sound_state(id),
    getSoundPosition:     (id)          => Deno.core.ops.bsengine_get_sound_position(id),
    // `id` is coerced to a string here: this op's Rust side takes a
    // #[string] id, and callers (see player.js/goal_levelN.js) pass a
    // plain numeric literal like `setHudText(1, ...)` — without this,
    // deno_core silently turns a non-string argument into an empty
    // string, so every numeric-id HUD slot collides on the same "" key.
    setHudText:     (id, text)             => Deno.core.ops.bsengine_set_hud_text(String(id), String(text)),
    clearHudText:   (id)                   => Deno.core.ops.bsengine_clear_hud_text(String(id)),

    // UI widgets — immediate-mode overlay (egui-backed)
    // Each call sets or replaces the widget with the given id.
    ui: {
        setLabel:    (id, text, x, y, fontSize)       => Deno.core.ops.bsengine_ui_set_label(id, String(text), x, y, fontSize ?? 20),
        setButton:   (id, label, x, y, width, height) => Deno.core.ops.bsengine_ui_set_button(id, label, x, y, width, height),
        setPanel:    (id, title, x, y, width, height) => Deno.core.ops.bsengine_ui_set_panel(id, title ?? '', x, y, width, height),
        setTextInput:(id, hint, x, y, width)          => Deno.core.ops.bsengine_ui_set_text_input(id, hint ?? '', x, y, width),
        remove:      (id)                             => Deno.core.ops.bsengine_ui_remove_widget(id),
        clear:       ()                               => Deno.core.ops.bsengine_ui_clear(),
        isClicked:   (id)                             => Deno.core.ops.bsengine_ui_is_clicked(id),
    },

    // NavMesh pathfinding — call navmesh.init() first to build the grid
    navmesh: {
        init:             (w, d, cs, ox, oy, oz) => Deno.core.ops.bsengine_navmesh_init(w, d, cs, ox ?? 0, oy ?? 0, oz ?? 0),
        setWalkable:      (x, z, v)              => Deno.core.ops.bsengine_navmesh_set_walkable(x, z, !!v),
        setDestination:   (name, x, y, z)        => Deno.core.ops.bsengine_set_nav_destination(name, x, y, z),
        clearDestination: (name)                 => Deno.core.ops.bsengine_clear_nav_destination(name),
        setSpeed:         (name, speed)          => Deno.core.ops.bsengine_set_nav_speed(name, speed),
        setEnabled:       (name, en)             => Deno.core.ops.bsengine_set_nav_enabled(name, !!en),
        getState:         (name)                 => Deno.core.ops.bsengine_navmesh_get_state(name),
        isMoving:         (name)                 => Deno.core.ops.bsengine_is_nav_moving(name),
        hasArrived:       (name)                 => Deno.core.ops.bsengine_has_nav_arrived(name),
        isIdle:           (name)                 => Deno.core.ops.bsengine_is_nav_idle(name),
        hasNoPath:        (name)                 => Deno.core.ops.bsengine_nav_has_no_path(name),
    },

    loadScene:      (path)                 => Deno.core.ops.bsengine_load_scene(path),

    save:           (path)                 => Deno.core.ops.bsengine_save_game(path ?? 'save.json'),
    load:           (path)                 => Deno.core.ops.bsengine_load_game(path ?? 'save.json'),

    // Mouse input (btn: 0=Left, 1=Right, 2=Middle)
    isMousePressed: (btn)  => Deno.core.ops.bsengine_is_mouse_pressed(btn),
    isMouseDown:    (btn)  => Deno.core.ops.bsengine_is_mouse_down(btn),
    isMouseUp:      (btn)  => Deno.core.ops.bsengine_is_mouse_up(btn),
    getMousePos:    ()     => { const v = Deno.core.ops.bsengine_get_mouse_pos(); return { x: v[0], y: v[1] }; },
    getMouseDelta:  ()     => { const v = Deno.core.ops.bsengine_get_mouse_delta(); return { x: v[0], y: v[1] }; },

    // Raycast: origin/{x,y,z}, dir/{x,y,z}, maxDist → {entityName, point, normal, distance} or null
    raycast:        (origin, dir, maxDist) =>
        Deno.core.ops.bsengine_raycast(origin.x, origin.y, origin.z, dir.x, dir.y, dir.z, maxDist),

    // Gamepad (btn: 0=South/A..15=DPadRight; side: 0=L2, 1=R2)
    isGamepadButton:     (btn)  => Deno.core.ops.bsengine_is_gamepad_button(btn),
    isGamepadButtonDown: (btn)  => Deno.core.ops.bsengine_is_gamepad_button_down(btn),
    isGamepadButtonUp:   (btn)  => Deno.core.ops.bsengine_is_gamepad_button_up(btn),
    getLeftStick:        ()     => { const v = Deno.core.ops.bsengine_get_left_stick(); return { x: v[0], y: v[1] }; },
    getRightStick:       ()     => { const v = Deno.core.ops.bsengine_get_right_stick(); return { x: v[0], y: v[1] }; },
    getGamepadTrigger:   (side) => Deno.core.ops.bsengine_get_gamepad_trigger(side),

    // Skybox
    setSkybox:           (path) => Deno.core.ops.bsengine_set_skybox(path),

    // Key event callbacks (event-based alternative to polling)
    _keyDownHandlers: {},
    _keyUpHandlers: {},
    onKeyDown(key, fn) { (this._keyDownHandlers[key] ??= []).push(fn); },
    onKeyUp(key, fn)   { (this._keyUpHandlers[key]   ??= []).push(fn); },
    _dispatchKeyEvents() {
        const keys = ['W','A','S','D','Space','Enter','Escape','Up','Down','Left','Right'];
        for (const key of keys) {
            if (Deno.core.ops.bsengine_is_key_down(key)) {
                for (const fn of (this._keyDownHandlers[key] || [])) {
                    try { fn(); } catch(e) { this.log('[keyDown:' + key + '] ' + e); }
                }
            }
            if (Deno.core.ops.bsengine_is_key_up(key)) {
                for (const fn of (this._keyUpHandlers[key] || [])) {
                    try { fn(); } catch(e) { this.log('[keyUp:' + key + '] ' + e); }
                }
            }
        }
    },

    // Mouse event callbacks (btn: 0=Left, 1=Right, 2=Middle)
    _mouseDownHandlers: {},
    _mouseUpHandlers: {},
    onMouseDown(btn, fn) { (this._mouseDownHandlers[btn] ??= []).push(fn); },
    onMouseUp(btn, fn)   { (this._mouseUpHandlers[btn]   ??= []).push(fn); },
    _dispatchMouseEvents() {
        for (let btn = 0; btn < 3; btn++) {
            if (Deno.core.ops.bsengine_is_mouse_down(btn)) {
                for (const fn of (this._mouseDownHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[mouseDown:' + btn + '] ' + e); }
                }
            }
            if (Deno.core.ops.bsengine_is_mouse_up(btn)) {
                for (const fn of (this._mouseUpHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[mouseUp:' + btn + '] ' + e); }
                }
            }
        }
    },

    // Gamepad event callbacks (btn: 0=South/A..15=DPadRight)
    _gamepadDownHandlers: {},
    _gamepadUpHandlers: {},
    onGamepadButtonDown(btn, fn) { (this._gamepadDownHandlers[btn] ??= []).push(fn); },
    onGamepadButtonUp(btn, fn)   { (this._gamepadUpHandlers[btn]   ??= []).push(fn); },
    _dispatchGamepadEvents() {
        for (let btn = 0; btn < 16; btn++) {
            if (Deno.core.ops.bsengine_is_gamepad_button_down(btn)) {
                for (const fn of (this._gamepadDownHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[gamepadDown:' + btn + '] ' + e); }
                }
            }
            if (Deno.core.ops.bsengine_is_gamepad_button_up(btn)) {
                for (const fn of (this._gamepadUpHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[gamepadUp:' + btn + '] ' + e); }
                }
            }
        }
    },

    // Timers — frame-based (1 frame ≈ 1 tick)
    _timers: [],
    _nextTimerId: 0,
    setTimeout(callback, frames) {
        const id = this._nextTimerId++;
        this._timers.push({ id, callback, remaining: frames });
        return id;
    },
    clearTimeout(id) {
        this._timers = this._timers.filter(t => t.id !== id);
    },
    _tickTimers() {
        const toFire = [];
        const keep = [];
        for (const t of this._timers) {
            t.remaining--;
            (t.remaining <= 0 ? toFire : keep).push(t);
        }
        this._timers = keep;
        for (const t of toFire) {
            try { t.callback(); } catch (e) { this.log('[timer] ' + e); }
        }
    },

    // Physics collision callbacks — keyed by entity name
    _collisionHandlers: {},
    onCollision(entityName, callback) {
        if (!this._collisionHandlers[entityName]) this._collisionHandlers[entityName] = [];
        this._collisionHandlers[entityName].push(callback);
    },
    _runCollisions(events) {
        for (const { nameA, nameB, started } of events) {
            for (const cb of (this._collisionHandlers[nameA] || [])) {
                try { cb(nameB, started); } catch (e) { this.log('[collision] ' + e); }
            }
            for (const cb of (this._collisionHandlers[nameB] || [])) {
                try { cb(nameA, started); } catch (e) { this.log('[collision] ' + e); }
            }
        }
    },

    // Per-entity script registry. Keys are entity bit-IDs (strings).
    _scripts: {},

    // --- Messaging ---
    _messageHandlers: {},

    // Register a handler for messages of `key` addressed to `entityName`.
    onMessage(entityName, key, fn) {
        const k = `${entityName}::${key}`;
        (this._messageHandlers[k] ??= []).push(fn);
    },

    // Dispatch a message synchronously to all handlers registered for `target`+`key`.
    sendMessage(target, key, data) {
        const handlers = this._messageHandlers[`${target}::${key}`] || [];
        for (const fn of handlers) {
            try { fn(data); } catch (e) { this.log(`[msg:${target}:${key}] ${e}`); }
        }
    },

    // Dispatch `key` to every entity that has a handler registered for it.
    broadcast(key, data) {
        const suffix = `::${key}`;
        for (const k of Object.keys(this._messageHandlers)) {
            if (k.endsWith(suffix)) {
                for (const fn of this._messageHandlers[k]) {
                    try { fn(data); } catch (e) { this.log(`[broadcast:${key}] ${e}`); }
                }
            }
        }
    },

    // Math utilities — pure JS, no round-trips to Rust.
    math: {
        lerp:      (a, b, t)  => a + (b - a) * t,
        clamp:     (v, lo, hi) => Math.min(Math.max(v, lo), hi),
        magnitude: (v)         => Math.sqrt(v.x*v.x + v.y*v.y + v.z*v.z),
        normalize: (v)         => { const l = Math.sqrt(v.x*v.x+v.y*v.y+v.z*v.z); return l>0?{x:v.x/l,y:v.y/l,z:v.z/l}:{x:0,y:0,z:0}; },
        dot:       (a, b)      => a.x*b.x + a.y*b.y + a.z*b.z,
        cross:     (a, b)      => ({x:a.y*b.z-a.z*b.y, y:a.z*b.x-a.x*b.z, z:a.x*b.y-a.y*b.x}),
        lerpVec:   (a, b, t)   => ({x:a.x+(b.x-a.x)*t, y:a.y+(b.y-a.y)*t, z:a.z+(b.z-a.z)*t}),
    },

    // Convenience helpers built on existing ops.
    lookAtEntity(name, targetName) {
        const t = this.getPosition(targetName);
        if (t) this.lookAt(name, t.x, t.y, t.z);
    },
    moveToward(name, tx, ty, tz, speed) {
        const pos = this.getPosition(name);
        if (!pos) return;
        const dx = tx-pos.x, dy = ty-pos.y, dz = tz-pos.z;
        const dist = Math.sqrt(dx*dx+dy*dy+dz*dz);
        if (dist < 1e-6) return;
        const step = Math.min(speed * this.getDeltaTime(), dist) / dist;
        this.setTransform(name, pos.x+dx*step, pos.y+dy*step, pos.z+dz*step);
    },
    getAngularSpeed(name) {
        const v = this.getAngularVelocity(name);
        return v ? Math.sqrt(v.x*v.x+v.y*v.y+v.z*v.z) : 0;
    },

    // Called each frame by the engine with [[id, name], ...] for all scripted entities.
    _runAll(entities) {
        this._tickTimers();
        this._dispatchKeyEvents();
        this._dispatchMouseEvents();
        this._dispatchGamepadEvents();
        for (const [id, name] of entities) {
            const s = this._scripts[id];
            if (s && s.onUpdate) {
                try {
                    s.onUpdate(name);
                } catch (e) {
                    this.log(`[${name}] onUpdate error: ${e}`);
                }
            }
        }
    },
};
"#;

#[cfg(test)]
mod tests {
    use crate::runtime::ScriptRuntime;

    #[test]
    fn op_log_callable_from_script() {
        let mut rt = ScriptRuntime::new_with_ops();
        let result = rt.eval(r#"Deno.core.ops.bsengine_log("hello from script"); "ok""#);
        assert!(result.is_ok(), "op call failed: {:?}", result);
        assert!(result.unwrap().contains("ok"));
    }

    #[test]
    fn op_version_returns_string() {
        let mut rt = ScriptRuntime::new_with_ops();
        let result = rt.eval(r#"Deno.core.ops.bsengine_version()"#);
        assert!(result.is_ok(), "version op failed: {:?}", result);
        let v = result.unwrap();
        assert!(v.contains("0.1"), "unexpected version: {v}");
    }

    #[test]
    fn bsengine_global_exposed_after_bootstrap() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"typeof Bsengine !== "undefined" ? "ok" : "missing""#)
            .unwrap();
        assert!(r.contains("ok"), "Bsengine global missing: {r}");
    }

    #[test]
    fn get_transform_returns_null_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getTransform("NoSuchEntity"))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn distance_to_returns_correct_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "A".to_string(),
                (
                    glam::Vec3::new(0.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "B".to_string(),
                (
                    glam::Vec3::new(3.0, 4.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.distanceTo("A", "B")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let dist: f32 = r.parse().expect("should be a number");
        assert!((dist - 5.0).abs() < 1e-4, "expected 5.0, got {dist}");
    }

    #[test]
    fn distance_to_returns_neg1_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.distanceTo("Ghost", "Unknown")"#)
            .unwrap();
        assert_eq!(r.trim(), "-1", "expected -1 for unknown: {r}");
    }

    #[test]
    fn distance_to_point_returns_correct_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"Bsengine.distanceToPoint("Player", 4.0, 0.0, 0.0)"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let dist: f32 = r.parse().expect("should be a number");
        assert!((dist - 3.0).abs() < 1e-4, "expected 3.0, got {dist}");
    }

    #[test]
    fn get_world_transform_returns_null_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getWorldTransform("NoSuchEntity"))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_forward_vector_returns_neg_z_for_identity_rotation() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getForwardVector("Player"))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(
            r.contains("-1") || r.contains("-0"),
            "expected -Z forward: {r}"
        );
    }

    #[test]
    fn get_world_position_reflects_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::WORLD_TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Child".to_string(),
                (
                    glam::Vec3::new(3.0, 4.0, 5.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getWorldPosition("Child"))"#)
            .unwrap();
        super::WORLD_TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(
            r.contains("\"x\":3") || r.contains("\"x\":3.0"),
            "pos x: {r}"
        );
        assert!(
            r.contains("\"y\":4") || r.contains("\"y\":4.0"),
            "pos y: {r}"
        );
        assert!(
            r.contains("\"z\":5") || r.contains("\"z\":5.0"),
            "pos z: {r}"
        );
    }

    #[test]
    fn get_right_vector_returns_x_for_identity_rotation() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getRightVector("Player"))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(
            r.contains("1") && !r.contains("-1"),
            "expected +X right: {r}"
        );
    }

    #[test]
    fn get_up_vector_returns_y_for_identity_rotation() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getUpVector("Player"))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("1"), "expected +Y up: {r}");
    }

    #[test]
    fn is_key_pressed_returns_false_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isKeyPressed("W") ? "pressed" : "not""#)
            .unwrap();
        assert!(r.contains("not"), "expected not pressed: {r}");
    }

    #[test]
    fn is_key_down_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isKeyDown("Space") ? "down" : "not""#)
            .unwrap();
        assert!(r.contains("not"), "expected not down: {r}");
    }

    #[test]
    fn is_key_up_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isKeyUp("Space") ? "up" : "not""#)
            .unwrap();
        assert!(r.contains("not"), "expected not up: {r}");
    }

    #[test]
    fn is_mouse_pressed_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isMousePressed(0) ? "yes" : "no""#)
            .unwrap();
        assert!(r.contains("no"), "expected not pressed: {r}");
    }

    #[test]
    fn get_mouse_pos_returns_zeros_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMousePos())"#)
            .unwrap();
        assert!(r.contains("0"), "expected zeros: {r}");
    }

    #[test]
    fn get_mouse_delta_returns_zeros_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMouseDelta())"#)
            .unwrap();
        assert!(r.contains("0"), "expected zeros: {r}");
    }

    #[test]
    fn raycast_returns_null_without_physics_world() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.raycast({x:0,y:0,z:0}, {x:0,y:-1,z:0}, 100.0))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null when no physics: {r}"
        );
    }

    #[test]
    fn set_timeout_fires_after_frames() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"
            let fired = false;
            Bsengine.setTimeout(() => { fired = true; }, 2);
            "#,
            "<test>",
        )
        .unwrap();
        rt.exec_source("Bsengine._runAll([]);", "<tick>").unwrap();
        let r = rt.eval("fired ? 'yes' : 'no'").unwrap();
        assert!(r.contains("no"), "should not fire on frame 1: {r}");
        rt.exec_source("Bsengine._runAll([]);", "<tick>").unwrap();
        let r = rt.eval("fired ? 'yes' : 'no'").unwrap();
        assert!(r.contains("yes"), "should fire on frame 2: {r}");
    }

    #[test]
    fn send_message_delivers_to_listener() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"
            let received = null;
            Bsengine.onMessage("Enemy", "hit", (data) => { received = data; });
            Bsengine.sendMessage("Enemy", "hit", 42);
            "#,
            "<test>",
        )
        .unwrap();
        let r = rt.eval("String(received)").unwrap();
        assert!(r.contains("42"), "expected 42: {r}");
    }

    #[test]
    fn is_gamepad_button_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isGamepadButton(0) ? "yes" : "no""#)
            .unwrap();
        assert!(r.contains("no"), "expected not pressed: {r}");
    }

    #[test]
    fn get_left_stick_returns_zeros_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getLeftStick())"#)
            .unwrap();
        assert!(r.contains("0"), "expected zeros: {r}");
    }

    #[test]
    fn get_gamepad_trigger_returns_zero_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.getGamepadTrigger(0))"#).unwrap();
        assert!(r.contains("0"), "expected 0: {r}");
    }

    #[test]
    fn on_collision_handler_dispatches() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"
            let hit = null;
            Bsengine.onCollision("Ball", (other, started) => { hit = other; });
            Bsengine._runCollisions([{nameA: "Ball", nameB: "Floor", started: true}]);
            "#,
            "<test>",
        )
        .unwrap();
        let r = rt.eval("hit").unwrap();
        assert!(r.contains("Floor"), "expected Floor: {r}");
    }

    #[test]
    fn set_parent_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setParent("Child", "Root");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetParent { child, parent }
                    if child == "Child" && parent == "Root")
            });
            assert!(found, "SetParent not in buffer");
        });
    }

    #[test]
    fn clear_parent_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.clearParent("Child");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::ClearParent { child } if child == "Child"));
            assert!(found, "ClearParent not in buffer");
        });
    }

    #[test]
    fn broadcast_fires_all_registered_handlers() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt
            .eval(
                r#"
let hits = 0;
Bsengine.onMessage("A", "boom", () => { hits++; });
Bsengine.onMessage("B", "boom", () => { hits++; });
Bsengine.onMessage("A", "other", () => { hits += 100; });
Bsengine.broadcast("boom", {});
hits
"#,
            )
            .unwrap();
        assert_eq!(result.trim(), "2", "expected 2 hits: {result}");
    }

    #[test]
    fn broadcast_no_op_when_no_handlers() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.broadcast("nobody", {}); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "threw: {r}");
    }

    #[test]
    fn math_lerp_midpoint() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval("Bsengine.math.lerp(0, 10, 0.5)").unwrap();
        let v: f32 = r.trim().parse().unwrap();
        assert!((v - 5.0).abs() < 1e-4, "expected 5.0: {r}");
    }

    #[test]
    fn math_normalize_unit_vector() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.math.normalize({x:3,y:4,z:0}))"#)
            .unwrap();
        assert!(
            r.contains("\"x\":0.6") || r.contains("\"x\":0.60"),
            "x: {r}"
        );
        assert!(
            r.contains("\"y\":0.8") || r.contains("\"y\":0.80"),
            "y: {r}"
        );
    }

    #[test]
    fn math_dot_product() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.math.dot({x:1,y:0,z:0},{x:0,y:1,z:0})"#)
            .unwrap();
        let v: f32 = r.trim().parse().unwrap();
        assert!((v - 0.0).abs() < 1e-4, "expected 0 for perpendicular: {r}");
    }

    #[test]
    fn get_screen_size_returns_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval("JSON.stringify(Bsengine.getScreenSize())").unwrap();
        assert!(r.contains("\"width\":1280"), "unexpected: {r}");
        assert!(r.contains("\"height\":720"), "unexpected: {r}");
    }

    #[test]
    fn messaging_delivers_to_handler() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt
            .eval(
                r#"
let received = null;
Bsengine.onMessage("Box", "hit", (data) => { received = data; });
Bsengine.sendMessage("Box", "hit", { damage: 5 });
JSON.stringify(received)
"#,
            )
            .unwrap();
        assert!(
            result.contains("\"damage\":5"),
            "message not delivered: {result}"
        );
    }

    #[test]
    fn messaging_no_op_for_unknown_recipient() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.sendMessage("NoOne", "event", {}); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "threw: {r}");
    }

    #[test]
    fn look_at_no_op_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // No transform snapshot → should not crash
        let r = rt
            .eval(r#"Bsengine.lookAt("NoEntity", 1, 0, 0); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "lookAt threw: {r}");
    }

    #[test]
    fn look_at_no_op_when_zero_dir() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Seed snapshot: entity at (1, 0, 0), target also (1, 0, 0) → zero dir
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Turret".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"Bsengine.lookAt("Turret", 1.0, 0.0, 0.0); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "lookAt zero-dir threw: {r}");
    }

    #[test]
    fn get_time_returns_zero_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // No time snapshot set → returns 0.0
        let r = rt
            .eval(r#"Bsengine.getTime() === 0.0 ? "zero" : "nonzero""#)
            .unwrap();
        assert!(r.contains("zero"), "expected 0.0: {r}");
    }

    #[test]
    fn get_delta_time_returns_zero_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getDeltaTime() === 0.0 ? "zero" : "nonzero""#)
            .unwrap();
        assert!(r.contains("zero"), "expected 0.0: {r}");
    }

    #[test]
    fn get_visible_returns_true_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Unknown entity defaults to visible=true
        let r = rt
            .eval(r#"Bsengine.getVisible("NonExistent") ? "visible" : "hidden""#)
            .unwrap();
        assert!(r.contains("visible"), "expected visible by default: {r}");
    }

    #[test]
    fn set_visible_queues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Should not throw
        let r = rt
            .eval(r#"Bsengine.setVisible("Cube", false); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "setVisible threw: {r}");
    }

    #[test]
    fn get_material_color_returns_null_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getMaterialColor("Ghost") === null ? "null" : "not-null""#)
            .unwrap();
        assert!(r.contains("null"), "expected null: {r}");
    }

    #[test]
    fn get_material_color_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_COLOR_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), [0.5, 0.25, 1.0]);
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMaterialColor("Cube"))"#)
            .unwrap();
        super::MATERIAL_COLOR_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.5"), "expected r=0.5: {r}");
    }

    #[test]
    fn get_material_emissive_returns_null_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getMaterialEmissive("Ghost") === null ? "null" : "not-null""#)
            .unwrap();
        assert!(r.contains("null"), "expected null: {r}");
    }

    #[test]
    fn get_material_emissive_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_EMISSIVE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), [0.75, 0.0, 0.0]);
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMaterialEmissive("Cube"))"#)
            .unwrap();
        super::MATERIAL_EMISSIVE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.75"), "expected r=0.75: {r}");
    }

    #[test]
    fn on_key_down_registers_and_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Register handler; since no key snapshot, handler must NOT be called
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onKeyDown('Space', () => { called = true; });
            Bsengine._dispatchKeyEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_key_up_registers_and_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onKeyUp('Enter', () => { called = true; });
            Bsengine._dispatchKeyEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn get_parent_returns_null_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.getParent("Child"))"#).unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_parent_returns_name_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::PARENT_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Child".to_string(), "Root".to_string());
        });
        let r = rt.eval(r#"Bsengine.getParent("Child")"#).unwrap();
        assert!(r.contains("Root"), "expected Root: {r}");
    }

    #[test]
    fn get_children_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getChildren("Root"))"#)
            .unwrap();
        assert!(r.contains("[]"), "expected empty array: {r}");
    }

    #[test]
    fn get_children_returns_list_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::CHILDREN_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Root".to_string(),
                vec!["ChildA".to_string(), "ChildB".to_string()],
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getChildren("Root"))"#)
            .unwrap();
        assert!(r.contains("ChildA"), "expected ChildA: {r}");
        assert!(r.contains("ChildB"), "expected ChildB: {r}");
    }

    #[test]
    fn entity_exists_returns_true_when_in_snapshot() {
        super::ENTITY_NAMES_SNAPSHOT.with(|s| {
            s.borrow_mut().push("Player".to_string());
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.entityExists("Player") ? "yes" : "no";"#)
            .unwrap();
        super::ENTITY_NAMES_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("yes"), "expected yes: {r}");
    }

    #[test]
    fn entity_exists_returns_false_for_unknown_name() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.entityExists("Ghost") ? "yes" : "no";"#)
            .unwrap();
        assert!(r.contains("no"), "expected no: {r}");
    }

    #[test]
    fn get_entity_count_returns_snapshot_length() {
        super::ENTITY_NAMES_SNAPSHOT.with(|s| {
            let mut v = s.borrow_mut();
            v.push("A".to_string());
            v.push("B".to_string());
            v.push("C".to_string());
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getEntityCount();"#).unwrap();
        super::ENTITY_NAMES_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains('3'), "expected 3: {r}");
    }

    #[test]
    fn get_closest_entity_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getClosestEntity(0, 0, 0)"#).unwrap();
        assert!(r.trim_matches('"').is_empty(), "expected empty: {r}");
    }

    #[test]
    fn get_closest_entity_returns_nearest() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "Near".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "Far".to_string(),
                (
                    glam::Vec3::new(100.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getClosestEntity(0, 0, 0)"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("Near"), "expected Near: {r}");
    }

    #[test]
    fn get_entities_in_radius_returns_nearby_entities() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "Near".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "Far".to_string(),
                (
                    glam::Vec3::new(100.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getEntitiesInRadius(0.0, 0.0, 0.0, 5.0))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("Near"), "expected Near: {r}");
        assert!(!r.contains("Far"), "should not contain Far: {r}");
    }

    #[test]
    fn get_entities_in_radius_returns_empty_when_none_in_range() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getEntitiesInRadius(0.0, 0.0, 0.0, 1.0))"#)
            .unwrap();
        assert_eq!(r.trim(), "[]", "expected empty array: {r}");
    }

    #[test]
    fn set_kinematic_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setKinematic("Box", true);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetKinematic { name, kinematic }
                    if name == "Box" && *kinematic)
            });
            assert!(found, "SetKinematic not in buffer");
        });
    }

    #[test]
    fn set_gravity_scale_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setGravityScale("Ball", 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetGravityScale { name, scale }
                    if name == "Ball" && (*scale - 0.5).abs() < 1e-6)
            });
            assert!(found, "SetGravityScale not in buffer");
        });
    }

    #[test]
    fn set_collider_sensor_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setColliderSensor("Zone", true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetColliderSensor { name, sensor }
                    if name == "Zone" && *sensor)
            });
            assert!(found, "SetColliderSensor not in buffer");
        });
    }

    #[test]
    fn get_velocity_returns_null_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.getVelocity("Ball"))"#).unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_velocity_returns_vec_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getVelocity("Ball"))"#)
            .unwrap();
        assert!(r.contains("\"x\":1"), "expected x=1: {r}");
        assert!(r.contains("\"y\":2"), "expected y=2: {r}");
    }

    #[test]
    fn add_impulse_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addImpulse("Ball", 0, 10, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddImpulse { name, fy, .. }
                    if name == "Ball" && (*fy - 10.0).abs() < 1e-6)
            });
            assert!(found, "AddImpulse not in buffer");
        });
    }

    #[test]
    fn apply_impulse_at_point_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.applyImpulseAtPoint("Ball", 0, 10, 0, 1, 0, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddImpulseAtPoint { name, fy, px, .. }
                    if name == "Ball" && (*fy - 10.0).abs() < 1e-6 && (*px - 1.0).abs() < 1e-6)
            });
            assert!(found, "AddImpulseAtPoint not in buffer");
        });
    }

    #[test]
    fn add_force_at_point_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addForceAtPoint("Ball", 0, 5, 0, 1, 0, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddForceAtPoint { name, fy, px, .. }
                    if name == "Ball" && (*fy - 5.0).abs() < 1e-6 && (*px - 1.0).abs() < 1e-6)
            });
            assert!(found, "AddForceAtPoint not in buffer");
        });
    }

    #[test]
    fn set_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocity("Ball", 5, 0, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocity { name, vx, .. }
                    if name == "Ball" && (*vx - 5.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocity not in buffer");
        });
    }

    #[test]
    fn set_velocity_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocityX("Ball", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocityX { name, vx }
                    if name == "Ball" && (*vx - 3.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocityX not in buffer");
        });
    }

    #[test]
    fn set_velocity_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocityY("Ball", 4.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocityY { name, vy }
                    if name == "Ball" && (*vy - 4.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocityY not in buffer");
        });
    }

    #[test]
    fn set_velocity_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocityZ("Ball", 2.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocityZ { name, vz }
                    if name == "Ball" && (*vz - 2.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocityZ not in buffer");
        });
    }

    #[test]
    fn add_position_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPosition("Player", 1, 2, 3);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddPosition { name, dx, dy, dz }
                    if name == "Player"
                        && (*dx - 1.0).abs() < 1e-6
                        && (*dy - 2.0).abs() < 1e-6
                        && (*dz - 3.0).abs() < 1e-6)
            });
            assert!(found, "AddPosition not in buffer");
        });
    }

    #[test]
    fn add_position_local_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionLocal("Player", 0, 0, -1);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddPositionLocal { name, dz, .. }
                    if name == "Player" && (*dz - (-1.0)).abs() < 1e-6)
            });
            assert!(found, "AddPositionLocal not in buffer");
        });
    }

    #[test]
    fn set_cursor_visible_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCursorVisible(false);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::SetCursorVisible { visible } if !visible),
            );
            assert!(found, "SetCursorVisible not in buffer");
        });
    }

    #[test]
    fn set_cursor_locked_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCursorLocked(true);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::SetCursorLocked { locked } if *locked),
            );
            assert!(found, "SetCursorLocked not in buffer");
        });
    }

    #[test]
    fn get_gravity_returns_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getGravity()"#).unwrap();
        assert!(
            r.contains("9.81") || r.contains("9.8"),
            "expected ~9.81: {r}"
        );
    }

    #[test]
    fn get_gravity_returns_snapshot_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::GRAVITY_SNAPSHOT.with(|s| *s.borrow_mut() = 20.0);
        let r = rt.eval(r#"Bsengine.getGravity()"#).unwrap();
        super::GRAVITY_SNAPSHOT.with(|s| *s.borrow_mut() = 9.81);
        assert!(r.contains("20"), "expected 20: {r}");
    }

    #[test]
    fn set_gravity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setGravity(0.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetGravity { magnitude }
                    if (*magnitude).abs() < 1e-6)
            });
            assert!(found, "SetGravity not in buffer");
        });
    }

    #[test]
    fn get_angular_velocity_returns_null_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getAngularVelocity("Spinner"))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_angular_velocity_returns_vec_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Spinner".to_string(), glam::Vec3::new(0.0, 2.5, 0.0));
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getAngularVelocity("Spinner"))"#)
            .unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("\"y\":2.5"), "expected y=2.5: {r}");
    }

    #[test]
    fn set_angular_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocity("Top", 0, 5, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocity { name, vy, .. }
                    if name == "Top" && (*vy - 5.0).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocity not in buffer");
        });
    }

    #[test]
    fn set_angular_velocity_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocityX("Top", 1.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocityX { name, vx }
                    if name == "Top" && (*vx - 1.5).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocityX not in buffer");
        });
    }

    #[test]
    fn set_angular_velocity_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocityY("Top", 2.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocityY { name, vy }
                    if name == "Top" && (*vy - 2.5).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocityY not in buffer");
        });
    }

    #[test]
    fn set_angular_velocity_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocityZ("Top", 3.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocityZ { name, vz }
                    if name == "Top" && (*vz - 3.5).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocityZ not in buffer");
        });
    }

    #[test]
    fn add_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addVelocity("Ball", 1.0, 2.0, 3.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddVelocity { name, vx, vy, vz }
                    if name == "Ball" && (*vx - 1.0).abs() < 1e-6
                        && (*vy - 2.0).abs() < 1e-6
                        && (*vz - 3.0).abs() < 1e-6)
            });
            assert!(found, "AddVelocity not in buffer");
        });
    }

    #[test]
    fn add_angular_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addAngularVelocity("Top", 0.1, 0.2, 0.3);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddAngularVelocity { name, vx, vy, vz }
                    if name == "Top" && (*vx - 0.1).abs() < 1e-6
                        && (*vy - 0.2).abs() < 1e-6
                        && (*vz - 0.3).abs() < 1e-6)
            });
            assert!(found, "AddAngularVelocity not in buffer");
        });
    }

    #[test]
    fn add_angular_impulse_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addAngularImpulse("Top", 0, 2, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddAngularImpulse { name, vy, .. }
                    if name == "Top" && (*vy - 2.0).abs() < 1e-6)
            });
            assert!(found, "AddAngularImpulse not in buffer");
        });
    }

    #[test]
    fn add_torque_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addTorque("Gyro", 0, 3, 0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddTorque { name, vy, .. }
                    if name == "Gyro" && (*vy - 3.0).abs() < 1e-6)
            });
            assert!(found, "AddTorque not in buffer");
        });
    }

    #[test]
    fn set_ccd_enabled_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCCDEnabled("Bullet", true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetCCDEnabled { name, enabled }
                    if name == "Bullet" && *enabled)
            });
            assert!(found, "SetCCDEnabled not in buffer");
        });
    }

    #[test]
    fn set_linear_damping_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setLinearDamping("Ball", 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetLinearDamping { name, damping }
                    if name == "Ball" && (*damping - 0.5).abs() < 1e-6)
            });
            assert!(found, "SetLinearDamping not in buffer");
        });
    }

    #[test]
    fn set_angular_damping_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularDamping("Ball", 0.8);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularDamping { name, damping }
                    if name == "Ball" && (*damping - 0.8).abs() < 1e-6)
            });
            assert!(found, "SetAngularDamping not in buffer");
        });
    }

    #[test]
    fn get_mass_returns_zero_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getMass("Rock")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_mass_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MASS_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Rock".to_string(), 5.0);
        });
        let r = rt.eval(r#"Bsengine.getMass("Rock")"#).unwrap();
        super::MASS_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains('5'), "expected 5: {r}");
    }

    #[test]
    fn get_gravity_scale_returns_default_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getGravityScale("Cube")"#).unwrap();
        assert!(r.contains('1'), "expected 1: {r}");
    }

    #[test]
    fn get_gravity_scale_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::GRAVITY_SCALE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), 2.5);
        });
        let r = rt.eval(r#"Bsengine.getGravityScale("Cube")"#).unwrap();
        super::GRAVITY_SCALE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("2.5"), "expected 2.5: {r}");
    }

    #[test]
    fn is_kinematic_returns_false_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.isKinematic("Cube"))"#).unwrap();
        assert_eq!(r, "false");
    }

    #[test]
    fn is_kinematic_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::BODY_TYPE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), true);
        });
        let r = rt.eval(r#"String(Bsengine.isKinematic("Cube"))"#).unwrap();
        super::BODY_TYPE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert_eq!(r, "true");
    }

    #[test]
    fn is_collider_sensor_returns_false_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.isColliderSensor("Zone"))"#)
            .unwrap();
        assert_eq!(r, "false");
    }

    #[test]
    fn is_collider_sensor_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::COLLIDER_SENSOR_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Zone".to_string(), true);
        });
        let r = rt
            .eval(r#"String(Bsengine.isColliderSensor("Zone"))"#)
            .unwrap();
        super::COLLIDER_SENSOR_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert_eq!(r, "true");
    }

    #[test]
    fn get_linear_damping_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getLinearDamping("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_linear_damping_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::LINEAR_DAMPING_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.3);
        });
        let r = rt.eval(r#"Bsengine.getLinearDamping("Ball")"#).unwrap();
        super::LINEAR_DAMPING_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.3"), "expected 0.3: {r}");
    }

    #[test]
    fn get_angular_damping_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getAngularDamping("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_angular_damping_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_DAMPING_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.75);
        });
        let r = rt.eval(r#"Bsengine.getAngularDamping("Ball")"#).unwrap();
        super::ANGULAR_DAMPING_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.75"), "expected 0.75: {r}");
    }

    #[test]
    fn get_restitution_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getRestitution("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_restitution_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::RESTITUTION_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.75);
        });
        let r = rt.eval(r#"Bsengine.getRestitution("Ball")"#).unwrap();
        super::RESTITUTION_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.75"), "expected 0.75: {r}");
    }

    #[test]
    fn set_restitution_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRestitution("Ball", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetRestitution { name, restitution }
                    if name == "Ball" && (*restitution - 0.5).abs() < 1e-6)
            });
            assert!(found, "SetRestitution not in buffer");
        });
    }

    #[test]
    fn get_friction_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getFriction("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_friction_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::FRICTION_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.5);
        });
        let r = rt.eval(r#"Bsengine.getFriction("Ball")"#).unwrap();
        super::FRICTION_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.5"), "expected 0.5: {r}");
    }

    #[test]
    fn set_friction_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setFriction("Ball", 0.25);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetFriction { name, friction }
                    if name == "Ball" && (*friction - 0.25).abs() < 1e-6)
            });
            assert!(found, "SetFriction not in buffer");
        });
    }

    #[test]
    fn set_mass_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setMass("Rock", 10.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetMass { name, mass }
                    if name == "Rock" && (*mass - 10.0).abs() < 1e-6)
            });
            assert!(found, "SetMass not in buffer");
        });
    }

    #[test]
    fn lock_rotation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.lockRotation("Player", true, false, true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::LockRotation { name, lock_x, lock_y, lock_z }
                    if name == "Player" && *lock_x && !*lock_y && *lock_z)
            });
            assert!(found, "LockRotation not in buffer");
        });
    }

    #[test]
    fn lock_translation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.lockTranslation("Player", false, true, false);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::LockTranslation { name, lock_x, lock_y, lock_z }
                    if name == "Player" && !*lock_x && *lock_y && !*lock_z)
            });
            assert!(found, "LockTranslation not in buffer");
        });
    }

    #[test]
    fn wake_up_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.wakeUp("Rock");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::WakeUp { name } if name == "Rock"));
            assert!(found, "WakeUp not in buffer");
        });
    }

    #[test]
    fn sleep_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.sleep("Rock");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::PutToSleep { name } if name == "Rock"),
            );
            assert!(found, "PutToSleep not in buffer");
        });
    }

    #[test]
    fn is_sleeping_returns_false_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let v = rt.eval(r#"String(Bsengine.isSleeping("Rock"))"#).unwrap();
        assert_eq!(v, "false");
    }

    #[test]
    fn is_sleeping_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::SLEEP_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Rock".to_string(), true);
        });
        let v = rt.eval(r#"String(Bsengine.isSleeping("Rock"))"#).unwrap();
        super::SLEEP_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert_eq!(v, "true");
    }

    #[test]
    fn on_mouse_down_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onMouseDown(0, () => { called = true; });
            Bsengine._dispatchMouseEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_mouse_down_called_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0b001); // btn 0 = Left
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onMouseDown(0, () => { called = true; });
            Bsengine._dispatchMouseEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        super::MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0);
        assert!(r.contains("called"), "expected called: {r}");
    }

    #[test]
    fn on_mouse_up_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onMouseUp(1, () => { called = true; });
            Bsengine._dispatchMouseEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_gamepad_button_down_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onGamepadButtonDown(0, () => { called = true; });
            Bsengine._dispatchGamepadEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_gamepad_button_down_called_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0b0001); // btn 0
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onGamepadButtonDown(0, () => { called = true; });
            Bsengine._dispatchGamepadEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        super::GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0);
        assert!(r.contains("called"), "expected called: {r}");
    }

    #[test]
    fn on_gamepad_button_up_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onGamepadButtonUp(3, () => { called = true; });
            Bsengine._dispatchGamepadEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn spawn_params_rotation_defaults_to_identity() {
        use crate::ops::SpawnParams;
        let p: SpawnParams =
            serde_json::from_str(r#"{"name":"Cube1","primitive":"Cube","x":0,"y":0,"z":0}"#)
                .unwrap();
        assert_eq!(p.rx, 0.0);
        assert_eq!(p.ry, 0.0);
        assert_eq!(p.rz, 0.0);
        assert_eq!(p.rw, 1.0, "rw should default to 1 (identity quaternion)");
    }

    #[test]
    fn spawn_params_rotation_accepted() {
        use crate::ops::SpawnParams;
        let p: SpawnParams = serde_json::from_str(
            r#"{"name":"Tilted","primitive":"Cube","x":0,"y":0,"z":0,
               "rx":0.0,"ry":0.707,"rz":0.0,"rw":0.707}"#,
        )
        .unwrap();
        assert!((p.ry - 0.707).abs() < 1e-3);
        assert!((p.rw - 0.707).abs() < 1e-3);
    }

    #[test]
    fn set_position_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPositionX("Player", 5.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetPositionX { name, x } => {
                    assert_eq!(name, "Player");
                    assert!((x - 5.0).abs() < 1e-4);
                }
                _ => panic!("expected SetPositionX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_position_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPositionY("Player", -3.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetPositionY { name, y } => {
                    assert_eq!(name, "Player");
                    assert!((y - (-3.0)).abs() < 1e-4);
                }
                _ => panic!("expected SetPositionY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_position_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPositionZ("Player", 10.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetPositionZ { name, z } => {
                    assert_eq!(name, "Player");
                    assert!((z - 10.0).abs() < 1e-4);
                }
                _ => panic!("expected SetPositionZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn rotate_by_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.rotateBy("Cube", 0.0, 0.707, 0.0, 0.707);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::RotateBy { name, ry, rw, .. } => {
                    assert_eq!(name, "Cube");
                    assert!((ry - 0.707).abs() < 1e-4);
                    assert!((rw - 0.707).abs() < 1e-4);
                }
                _ => panic!("expected RotateBy command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn rotate_around_axis_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.rotateAroundAxis("Cube", 0.0, 1.0, 0.0, 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::RotateAroundAxis {
                    name,
                    ay,
                    angle_deg,
                    ..
                } => {
                    assert_eq!(name, "Cube");
                    assert!((ay - 1.0).abs() < 1e-4);
                    assert!((angle_deg - 90.0).abs() < 1e-4);
                }
                _ => panic!("expected RotateAroundAxis command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_scale_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setScaleX("Cube", 2.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetScaleX { name, x } => {
                    assert_eq!(name, "Cube");
                    assert!((x - 2.0).abs() < 1e-4);
                }
                _ => panic!("expected SetScaleX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_scale_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setScaleY("Cube", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetScaleY { name, y } => {
                    assert_eq!(name, "Cube");
                    assert!((y - 3.0).abs() < 1e-4);
                }
                _ => panic!("expected SetScaleY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_scale_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setScaleZ("Cube", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetScaleZ { name, z } => {
                    assert_eq!(name, "Cube");
                    assert!((z - 0.5).abs() < 1e-4);
                }
                _ => panic!("expected SetScaleZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_linear_speed_returns_neg1_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let raw = rt
            .eval(r#"String(Deno.core.ops.bsengine_get_linear_speed("__no_such__"))"#)
            .unwrap();
        assert!(
            raw.contains("null") || raw.contains("undefined"),
            "op should return null for unknown entity: {raw}"
        );
        let wrapped = rt
            .eval(r#"Bsengine.getLinearSpeed("__no_such__")"#)
            .unwrap();
        let v: f32 = wrapped.trim().parse().expect("should be a number");
        assert!(
            v < 0.0,
            "wrapper should return -1 for unknown entity, got: {v}"
        );
    }

    #[test]
    fn get_linear_speed_returns_magnitude() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(3.0, 4.0, 0.0));
        });
        let r = rt.eval(r#"Bsengine.getLinearSpeed("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let speed: f32 = r.trim().parse().expect("expected a number");
        assert!((speed - 5.0).abs() < 1e-4, "expected 5.0, got {speed}");
    }

    #[test]
    fn get_children_count_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getChildrenCount("NoEntity")"#).unwrap();
        assert_eq!(r.trim(), "0", "expected 0: {r}");
    }

    #[test]
    fn get_child_at_returns_null_for_out_of_bounds() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getChildAt("NoEntity", 0))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_child_at_returns_correct_child() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::CHILDREN_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Root".to_string(),
                vec!["ChildA".to_string(), "ChildB".to_string()],
            );
        });
        let r = rt.eval(r#"Bsengine.getChildAt("Root", 1)"#).unwrap();
        super::CHILDREN_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("ChildB"), "expected ChildB: {r}");
    }

    #[test]
    fn set_rotation_euler_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEuler("Cube", 45.0, 90.0, 0.0);"#)
            .unwrap();

        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEuler {
                    name,
                    pitch_deg,
                    yaw_deg,
                    roll_deg,
                } => {
                    assert_eq!(name, "Cube");
                    assert!((pitch_deg - 45.0).abs() < 1e-4);
                    assert!((yaw_deg - 90.0).abs() < 1e-4);
                    assert!((roll_deg - 0.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEuler command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEuler("Cube", 30.0, 45.0, 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddRotationEuler {
                    name,
                    pitch,
                    yaw,
                    roll,
                } => {
                    assert_eq!(name, "Cube");
                    assert!((pitch - 30.0).abs() < 1e-4, "pitch: {pitch}");
                    assert!((yaw - 45.0).abs() < 1e-4, "yaw: {yaw}");
                    assert!((roll - 90.0).abs() < 1e-4, "roll: {roll}");
                }
                _ => panic!("expected AddRotationEuler command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEulerX("Cube", 45.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddRotationEulerX { name, deg }
                    if name == "Cube" && (*deg - 45.0).abs() < 1e-4)
            });
            assert!(found, "AddRotationEulerX not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEulerY("Cube", 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddRotationEulerY { name, deg }
                    if name == "Cube" && (*deg - 90.0).abs() < 1e-4)
            });
            assert!(found, "AddRotationEulerY not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEulerZ("Cube", 30.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddRotationEulerZ { name, deg }
                    if name == "Cube" && (*deg - 30.0).abs() < 1e-4)
            });
            assert!(found, "AddRotationEulerZ not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn pause_sound_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.pauseSound(42);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::PauseSound { id } if *id == 42));
            assert!(found, "PauseSound not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn resume_sound_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.resumeSound(7);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::ResumeSound { id } if *id == 7));
            assert!(found, "ResumeSound not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_sound_volume_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSoundVolume(5, -6.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSoundVolume { id, db }
                    if *id == 5 && (*db - (-6.0_f32)).abs() < 1e-5)
            });
            assert!(found, "SetSoundVolume not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_sound_panning_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSoundPanning(3, -0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSoundPanning { id, panning }
                    if *id == 3 && (*panning - (-0.5_f32)).abs() < 1e-5)
            });
            assert!(found, "SetSoundPanning not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_sound_playback_rate_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSoundPlaybackRate(8, 2.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSoundPlaybackRate { id, rate }
                    if *id == 8 && (*rate - 2.0_f32).abs() < 1e-5)
            });
            assert!(found, "SetSoundPlaybackRate not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn seek_sound_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.seekSound(11, 2.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SeekSound { id, position }
                    if *id == 11 && (*position - 2.5_f64).abs() < 1e-9)
            });
            assert!(found, "SeekSound not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_sound_state_reads_snapshot() {
        super::SOUND_STATE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(99, "playing".to_string());
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getSoundState(99);"#).unwrap();
        super::SOUND_STATE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("playing"), "expected playing: {r}");
    }

    #[test]
    fn get_sound_state_returns_empty_for_unknown_id() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getSoundState(9999);"#).unwrap();
        assert!(
            r.trim().is_empty() || r.trim() == "\"\"",
            "expected empty string: {r}"
        );
    }

    #[test]
    fn get_sound_position_reads_snapshot() {
        super::SOUND_POSITION_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(42, 3.5);
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getSoundPosition(42);"#).unwrap();
        super::SOUND_POSITION_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("3.5"), "expected 3.5: {r}");
    }

    #[test]
    fn add_scale_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScale("Obj", 0.5, 0.5, 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScale { name, sx, sy, sz } => {
                    assert_eq!(name, "Obj");
                    assert!((sx - 0.5).abs() < 1e-4, "sx: {sx}");
                    assert!((sy - 0.5).abs() < 1e-4, "sy: {sy}");
                    assert!((sz - 0.5).abs() < 1e-4, "sz: {sz}");
                }
                _ => panic!("expected AddScale command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_zero_enqueues_identity_angles() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEuler("Box", 0.0, 0.0, 0.0);"#)
            .unwrap();

        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEuler {
                    pitch_deg,
                    yaw_deg,
                    roll_deg,
                    ..
                } => {
                    assert!((pitch_deg).abs() < 1e-4);
                    assert!((yaw_deg).abs() < 1e-4);
                    assert!((roll_deg).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEuler command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEulerX("Box", 45.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEulerX { name, deg } => {
                    assert_eq!(name, "Box");
                    assert!((deg - 45.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEulerX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEulerY("Box", 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEulerY { name, deg } => {
                    assert_eq!(name, "Box");
                    assert!((deg - 90.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEulerY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEulerZ("Box", 180.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEulerZ { name, deg } => {
                    assert_eq!(name, "Box");
                    assert!((deg - 180.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEulerZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn multiply_scale_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.multiplyScale("Obj", 2.0, 3.0, 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::MultiplyScale { name, sx, sy, sz } => {
                    assert_eq!(name, "Obj");
                    assert!((sx - 2.0).abs() < 1e-4);
                    assert!((sy - 3.0).abs() < 1e-4);
                    assert!((sz - 0.5).abs() < 1e-4);
                }
                _ => panic!("expected MultiplyScale command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_position_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::new(3.0, 5.0, 7.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getPositionX("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 3.0).abs() < 1e-4, "expected 3.0, got {v}");
    }

    #[test]
    fn get_position_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::new(3.0, 5.0, 7.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getPositionY("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 5.0).abs() < 1e-4, "expected 5.0, got {v}");
    }

    #[test]
    fn get_position_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::new(3.0, 5.0, 7.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getPositionZ("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 7.0).abs() < 1e-4, "expected 7.0, got {v}");
    }

    #[test]
    fn get_scale_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getScaleX("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 2.0).abs() < 1e-4, "expected 2.0, got {v}");
    }

    #[test]
    fn get_scale_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getScaleY("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 3.0).abs() < 1e-4, "expected 3.0, got {v}");
    }

    #[test]
    fn get_scale_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getScaleZ("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 4.0).abs() < 1e-4, "expected 4.0, got {v}");
    }

    #[test]
    fn get_rotation_euler_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, 30f32.to_radians(), 0.0, 0.0),
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getRotationEulerX("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 30.0).abs() < 1e-3, "expected 30.0, got {v}");
    }

    #[test]
    fn get_rotation_euler_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, 0.0, 45f32.to_radians(), 0.0),
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getRotationEulerY("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 45.0).abs() < 1e-3, "expected 45.0, got {v}");
    }

    #[test]
    fn get_rotation_euler_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, 0.0, 0.0, 90f32.to_radians()),
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getRotationEulerZ("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 90.0).abs() < 1e-3, "expected 90.0, got {v}");
    }

    #[test]
    fn get_velocity_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(5.0, 2.0, -3.0));
        });
        let r = rt.eval(r#"Bsengine.getVelocityX("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 5.0).abs() < 1e-4, "expected 5.0, got {v}");
    }

    #[test]
    fn get_velocity_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(5.0, 2.0, -3.0));
        });
        let r = rt.eval(r#"Bsengine.getVelocityY("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 2.0).abs() < 1e-4, "expected 2.0, got {v}");
    }

    #[test]
    fn get_velocity_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(5.0, 2.0, -3.0));
        });
        let r = rt.eval(r#"Bsengine.getVelocityZ("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - (-3.0)).abs() < 1e-4, "expected -3.0, got {v}");
    }

    #[test]
    fn get_angular_velocity_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Wheel".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt.eval(r#"Bsengine.getAngularVelocityX("Wheel")"#).unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 1.0).abs() < 1e-4, "expected 1.0, got {v}");
    }

    #[test]
    fn get_angular_velocity_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Wheel".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt.eval(r#"Bsengine.getAngularVelocityY("Wheel")"#).unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 2.0).abs() < 1e-4, "expected 2.0, got {v}");
    }

    #[test]
    fn get_angular_velocity_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Wheel".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt.eval(r#"Bsengine.getAngularVelocityZ("Wheel")"#).unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 3.0).abs() < 1e-4, "expected 3.0, got {v}");
    }

    #[test]
    fn add_position_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionX("Player", 5.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddPositionX { name, dx } => {
                    assert_eq!(name, "Player");
                    assert!((dx - 5.0).abs() < 1e-4);
                }
                _ => panic!("expected AddPositionX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_position_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionY("Player", -2.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddPositionY { name, dy } => {
                    assert_eq!(name, "Player");
                    assert!((dy - (-2.0)).abs() < 1e-4);
                }
                _ => panic!("expected AddPositionY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_position_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionZ("Player", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddPositionZ { name, dz } => {
                    assert_eq!(name, "Player");
                    assert!((dz - 3.0).abs() < 1e-4);
                }
                _ => panic!("expected AddPositionZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScaleX("Box", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScaleX { name, dx } => {
                    assert_eq!(name, "Box");
                    assert!((dx - 0.5).abs() < 1e-4);
                }
                _ => panic!("expected AddScaleX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScaleY("Box", 1.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScaleY { name, dy } => {
                    assert_eq!(name, "Box");
                    assert!((dy - 1.0).abs() < 1e-4);
                }
                _ => panic!("expected AddScaleY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScaleZ("Box", -0.25);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScaleZ { name, dz } => {
                    assert_eq!(name, "Box");
                    assert!((dz - (-0.25)).abs() < 1e-4);
                }
                _ => panic!("expected AddScaleZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_metallic_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setMetallic("Sphere", 0.8);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetMetallic { name, value } => {
                    assert_eq!(name, "Sphere");
                    assert!((value - 0.8).abs() < 1e-4);
                }
                _ => panic!("expected SetMetallic command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_metallic_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_METALLIC_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Sphere".to_string(), 0.75);
        });
        let r = rt.eval(r#"Bsengine.getMetallic("Sphere")"#).unwrap();
        super::MATERIAL_METALLIC_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 0.75).abs() < 1e-4, "expected 0.75, got {v}");
    }

    #[test]
    fn set_roughness_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRoughness("Sphere", 0.3);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRoughness { name, value } => {
                    assert_eq!(name, "Sphere");
                    assert!((value - 0.3).abs() < 1e-4);
                }
                _ => panic!("expected SetRoughness command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_roughness_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Sphere".to_string(), 0.4);
        });
        let r = rt.eval(r#"Bsengine.getRoughness("Sphere")"#).unwrap();
        super::MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 0.4).abs() < 1e-4, "expected 0.4, got {v}");
    }

    #[test]
    fn set_point_light_color_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPointLightColor("Lamp", 1.0, 0.5, 0.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetPointLightColor { name, r, g, b }
                    if name == "Lamp" && (*r - 1.0).abs() < 1e-5 && (*g - 0.5).abs() < 1e-5 && b.abs() < 1e-5)
            });
            assert!(found, "SetPointLightColor not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_point_light_intensity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPointLightIntensity("Lamp", 3.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetPointLightIntensity { name, value }
                    if name == "Lamp" && (*value - 3.0).abs() < 1e-5)
            });
            assert!(found, "SetPointLightIntensity not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_point_light_range_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPointLightRange("Lamp", 20.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetPointLightRange { name, value }
                    if name == "Lamp" && (*value - 20.0).abs() < 1e-5)
            });
            assert!(found, "SetPointLightRange not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_spot_light_color_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSpotLightColor("Spot", 1.0, 0.5, 0.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSpotLightColor { name, r, g, b }
                    if name == "Spot"
                        && (*r - 1.0).abs() < 1e-5
                        && (*g - 0.5).abs() < 1e-5
                        && (*b - 0.0).abs() < 1e-5)
            });
            assert!(found, "SetSpotLightColor not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_spot_light_intensity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSpotLightIntensity("Spot", 800.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSpotLightIntensity { name, value }
                    if name == "Spot" && (*value - 800.0).abs() < 1e-5)
            });
            assert!(found, "SetSpotLightIntensity not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_spot_light_range_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSpotLightRange("Spot", 15.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSpotLightRange { name, value }
                    if name == "Spot" && (*value - 15.0).abs() < 1e-5)
            });
            assert!(found, "SetSpotLightRange not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_spot_light_inner_angle_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSpotLightInnerAngle("Spot", 30.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSpotLightInnerAngle { name, deg }
                    if name == "Spot" && (*deg - 30.0).abs() < 1e-5)
            });
            assert!(found, "SetSpotLightInnerAngle not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_spot_light_outer_angle_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setSpotLightOuterAngle("Spot", 45.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetSpotLightOuterAngle { name, deg }
                    if name == "Spot" && (*deg - 45.0).abs() < 1e-5)
            });
            assert!(found, "SetSpotLightOuterAngle not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_directional_light_color_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setDirectionalLightColor("Sun", 1.0, 0.9, 0.8);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetDirectionalLightColor { name, r, g, b }
                    if name == "Sun"
                        && (*r - 1.0).abs() < 1e-5
                        && (*g - 0.9).abs() < 1e-5
                        && (*b - 0.8).abs() < 1e-5)
            });
            assert!(found, "SetDirectionalLightColor not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_directional_light_ambient_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setDirectionalLightAmbient("Sun", 0.1, 0.1, 0.15);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetDirectionalLightAmbient { name, r, g, b }
                    if name == "Sun"
                        && (*r - 0.1).abs() < 1e-5
                        && (*g - 0.1).abs() < 1e-5
                        && (*b - 0.15).abs() < 1e-5)
            });
            assert!(found, "SetDirectionalLightAmbient not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_directional_light_direction_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setDirectionalLightDirection("Sun", -0.4, -0.8, -0.4);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetDirectionalLightDirection { name, x, y, z }
                    if name == "Sun"
                        && (*x - -0.4).abs() < 1e-5
                        && (*y - -0.8).abs() < 1e-5
                        && (*z - -0.4).abs() < 1e-5)
            });
            assert!(found, "SetDirectionalLightDirection not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_camera_fov_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCameraFov("MainCamera", 75.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetCameraFov { name, deg }
                    if name == "MainCamera" && (*deg - 75.0).abs() < 1e-5)
            });
            assert!(found, "SetCameraFov not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_camera_near_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCameraNear("MainCamera", 0.01);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetCameraNear { name, value }
                    if name == "MainCamera" && (*value - 0.01).abs() < 1e-5)
            });
            assert!(found, "SetCameraNear not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_camera_far_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCameraFar("MainCamera", 2000.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetCameraFar { name, value }
                    if name == "MainCamera" && (*value - 2000.0).abs() < 1e-5)
            });
            assert!(found, "SetCameraFar not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_damping_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setDamping("Ball", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetDamping { name, value }
                    if name == "Ball" && (*value - 0.5).abs() < 1e-5)
            });
            assert!(found, "SetDamping not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn play_animation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.playAnimation("Hero", "walk");"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::PlayAnimation { name, clip }
                    if name == "Hero" && clip == "walk")
            });
            assert!(found, "PlayAnimation not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn pause_animation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.pauseAnimation("Hero");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::PauseAnimation { name }
                    if name == "Hero")
            });
            assert!(found, "PauseAnimation not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn resume_animation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.resumeAnimation("Hero");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::ResumeAnimation { name }
                    if name == "Hero")
            });
            assert!(found, "ResumeAnimation not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn reset_animation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.resetAnimation("Hero");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::ResetAnimation { name }
                    if name == "Hero")
            });
            assert!(found, "ResetAnimation not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_animation_speed_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAnimationSpeed("Hero", 2.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAnimationSpeed { name, speed }
                    if name == "Hero" && (*speed - 2.0).abs() < 1e-5)
            });
            assert!(found, "SetAnimationSpeed not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_animation_looping_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAnimationLooping("Hero", false);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAnimationLooping { name, looping }
                    if name == "Hero" && !*looping)
            });
            assert!(found, "SetAnimationLooping not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_animation_clip_returns_empty_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getAnimationClip("Unknown");"#).unwrap();
        assert!(
            r.trim().is_empty() || r.trim() == "\"\"",
            "expected empty, got {r}"
        );
    }

    #[test]
    fn get_animation_time_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getAnimationTime("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn get_animation_speed_returns_one_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getAnimationSpeed("Unknown");"#)
            .unwrap();
        assert!(r.trim() == "1" || r.trim() == "1.0", "expected 1, got {r}");
    }

    #[test]
    fn is_animation_playing_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isAnimationPlaying("Unknown");"#)
            .unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn is_animation_looping_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isAnimationLooping("Unknown");"#)
            .unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn set_lifetime_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setLifetime("Bullet", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetLifetime { name, seconds }
                    if name == "Bullet" && (*seconds - 3.0).abs() < 1e-5)
            });
            assert!(found, "SetLifetime not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_lifetime_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getLifetime("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn damage_shield_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.damageShield("Hero", 20.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::DamageShield { name, amount }
                    if name == "Hero" && (*amount - 20.0).abs() < 1e-5)
            });
            assert!(found, "DamageShield not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn restore_shield_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.restoreShield("Hero", 15.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::RestoreShield { name, amount }
                    if name == "Hero" && (*amount - 15.0).abs() < 1e-5)
            });
            assert!(found, "RestoreShield not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_max_shield_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setMaxShield("Hero", 100.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetMaxShield { name, value }
                    if name == "Hero" && (*value - 100.0).abs() < 1e-5)
            });
            assert!(found, "SetMaxShield not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_shield_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getShield("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn get_max_shield_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getMaxShield("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn get_shield_fraction_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getShieldFraction("Unknown");"#)
            .unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn is_shield_depleted_returns_true_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.isShieldDepleted("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "true");
    }

    #[test]
    fn reset_timer_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.resetTimer("Clock");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::ResetTimer { name } if name == "Clock"),
            );
            assert!(found, "ResetTimer not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_timer_elapsed_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getTimerElapsed("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn get_timer_fraction_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getTimerFraction("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn is_timer_finished_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.isTimerFinished("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn is_timer_just_finished_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isTimerJustFinished("Unknown");"#)
            .unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn set_nav_destination_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setNavDestination("Enemy", 10.0, 0.0, 5.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetNavDestination { name, x, .. }
                    if name == "Enemy" && (*x - 10.0).abs() < 1e-5)
            });
            assert!(found, "SetNavDestination not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn clear_nav_destination_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.clearNavDestination("Enemy");"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::ClearNavDestination { name } if name == "Enemy")
            });
            assert!(found, "ClearNavDestination not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_nav_speed_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getNavSpeed("Unknown");"#).unwrap();
        assert!(r.trim() == "0" || r.trim() == "0.0", "expected 0, got {r}");
    }

    #[test]
    fn is_nav_moving_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.isNavMoving("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn has_nav_arrived_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.hasNavArrived("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn is_nav_idle_returns_true_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.isNavIdle("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "true");
    }

    #[test]
    fn nav_has_no_path_returns_false_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.navHasNoPath("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "false");
    }

    #[test]
    fn is_nav_enabled_returns_true_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.isNavEnabled("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "true");
    }

    #[test]
    fn navmesh_init_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.navmesh.init(20, 20, 1.0, -10, 0, -10);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::NavmeshInit { width, depth, .. }
                    if *width == 20 && *depth == 20)
            });
            assert!(found, "NavmeshInit not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn navmesh_set_walkable_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.navmesh.setWalkable(3, 5, false);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::NavmeshSetWalkable { x, z, walkable }
                    if *x == 3 && *z == 5 && !walkable)
            });
            assert!(found, "NavmeshSetWalkable not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn navmesh_get_state_returns_idle_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.navmesh.getState("Unknown");"#).unwrap();
        assert_eq!(r.trim(), "idle");
    }

    #[test]
    fn navmesh_set_destination_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.navmesh.setDestination("Agent", 5.0, 0.0, 3.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetNavDestination { name, x, .. }
                    if name == "Agent" && (*x - 5.0).abs() < 1e-5)
            });
            assert!(
                found,
                "SetNavDestination via navmesh.setDestination not in buffer"
            );
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn save_game_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.save("mysave.json");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::SaveGame { path } if path == "mysave.json"));
            assert!(found, "SaveGame not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn load_game_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.load("mysave.json");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::LoadGame { path } if path == "mysave.json"));
            assert!(found, "LoadGame not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_custom_shader_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setShader("cube", "shaders/wave.wgsl");"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetCustomShader { name, path }
                    if name == "cube" && path == "shaders/wave.wgsl")
            });
            assert!(found, "SetCustomShader not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn clear_custom_shader_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.clearShader("cube");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::ClearCustomShader { name } if name == "cube"));
            assert!(found, "ClearCustomShader not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn bloom_snapshot_read_ops() {
        super::BLOOM_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Cam".to_string(), (0.5, 1.0, 4.0, 0.5, true));
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let intensity = rt.eval(r#"Bsengine.getBloomIntensity("Cam");"#).unwrap();
        assert!((intensity.trim().parse::<f32>().unwrap() - 0.5).abs() < 0.001);
        let threshold = rt.eval(r#"Bsengine.getBloomThreshold("Cam");"#).unwrap();
        assert!((threshold.trim().parse::<f32>().unwrap() - 1.0).abs() < 0.001);
        let radius = rt.eval(r#"Bsengine.getBloomRadius("Cam");"#).unwrap();
        assert!((radius.trim().parse::<f32>().unwrap() - 4.0).abs() < 0.001);
        let softness = rt.eval(r#"Bsengine.getBloomSoftness("Cam");"#).unwrap();
        assert!((softness.trim().parse::<f32>().unwrap() - 0.5).abs() < 0.001);
        let en = rt.eval(r#"Bsengine.isBloomEnabled("Cam");"#).unwrap();
        assert_eq!(en.trim(), "true");
        super::BLOOM_SNAPSHOT.with(|s| s.borrow_mut().remove("Cam"));
    }

    #[test]
    fn bloom_write_ops_queue_commands() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setBloomIntensity("Cam", 0.8);"#)
            .unwrap();
        rt.eval(r#"Bsengine.setBloomThreshold("Cam", 0.9);"#)
            .unwrap();
        rt.eval(r#"Bsengine.setBloomRadius("Cam", 6.0);"#).unwrap();
        rt.eval(r#"Bsengine.setBloomSoftness("Cam", 0.3);"#)
            .unwrap();
        rt.eval(r#"Bsengine.setBloomEnabled("Cam", false);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetBloomIntensity { name, intensity }
                if name == "Cam" && (*intensity - 0.8).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetBloomThreshold { name, threshold }
                if name == "Cam" && (*threshold - 0.9).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetBloomRadius { name, radius }
                if name == "Cam" && (*radius - 6.0).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetBloomSoftness { name, softness }
                if name == "Cam" && (*softness - 0.3).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetBloomEnabled { name, enabled }
                if name == "Cam" && !*enabled
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn ao_snapshot_read_ops() {
        super::AMBIENT_OCCLUSION_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Cam".to_string(), (0.5, 0.025, 0.8, 8u32, true));
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let radius = rt.eval(r#"Bsengine.getAoRadius("Cam");"#).unwrap();
        assert!((radius.trim().parse::<f32>().unwrap() - 0.5).abs() < 0.001);
        let bias = rt.eval(r#"Bsengine.getAoBias("Cam");"#).unwrap();
        assert!((bias.trim().parse::<f32>().unwrap() - 0.025).abs() < 0.001);
        let intensity = rt.eval(r#"Bsengine.getAoIntensity("Cam");"#).unwrap();
        assert!((intensity.trim().parse::<f32>().unwrap() - 0.8).abs() < 0.001);
        let count = rt.eval(r#"Bsengine.getAoSampleCount("Cam");"#).unwrap();
        assert_eq!(count.trim(), "8");
        let en = rt.eval(r#"Bsengine.isAoEnabled("Cam");"#).unwrap();
        assert_eq!(en.trim(), "true");
        super::AMBIENT_OCCLUSION_SNAPSHOT.with(|s| s.borrow_mut().remove("Cam"));
    }

    #[test]
    fn ao_write_ops_queue_commands() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAoRadius("Cam", 0.5);"#).unwrap();
        rt.eval(r#"Bsengine.setAoBias("Cam", 0.025);"#).unwrap();
        rt.eval(r#"Bsengine.setAoIntensity("Cam", 0.8);"#).unwrap();
        rt.eval(r#"Bsengine.setAoSampleCount("Cam", 16);"#).unwrap();
        rt.eval(r#"Bsengine.setAoEnabled("Cam", false);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetAoRadius { name, radius }
                if name == "Cam" && (*radius - 0.5).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetAoSampleCount { name, count }
                if name == "Cam" && *count == 16
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetAoEnabled { name, enabled }
                if name == "Cam" && !*enabled
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_tone_map_read_ops() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();

        // Aces=3
        super::TONE_MAP_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cam".to_string(), (3, 0.0, true));
        });

        let mode = rt.eval(r#"Bsengine.getToneMapMode("Cam");"#).unwrap();
        assert_eq!(mode.trim(), "3");
        let exp = rt.eval(r#"Bsengine.getToneMapExposure("Cam");"#).unwrap();
        assert!((exp.trim().parse::<f32>().unwrap() - 0.0).abs() < 0.001);
        let en = rt.eval(r#"Bsengine.isToneMapEnabled("Cam");"#).unwrap();
        assert_eq!(en.trim(), "true");

        super::TONE_MAP_SNAPSHOT.with(|s| s.borrow_mut().remove("Cam"));
    }

    #[test]
    fn test_tone_map_write_ops_queue_commands() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setToneMapMode("Cam", 1);"#).unwrap();
        rt.eval(r#"Bsengine.setToneMapExposure("Cam", 1.5);"#)
            .unwrap();
        rt.eval(r#"Bsengine.setToneMapEnabled("Cam", false);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetToneMapMode { name, mode }
                if name == "Cam" && *mode == 1
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetToneMapExposure { name, exposure }
                if name == "Cam" && (*exposure - 1.5).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetToneMapEnabled { name, enabled }
                if name == "Cam" && !*enabled
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_tween_read_ops() {
        // (target_type=2/Scale, duration=3.0, easing=2/EaseOutQuad, repeat=1/Loop, elapsed=1.5, finished=false, reversed=true)
        super::TWEEN_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Box".to_string(), (2u32, 3.0, 2u32, 1u32, 1.5, false, true));
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let tt = rt.eval(r#"Bsengine.getTweenTargetType("Box");"#).unwrap();
        assert_eq!(tt.trim().parse::<u32>().unwrap(), 2u32);
        let dur = rt.eval(r#"Bsengine.getTweenDuration("Box");"#).unwrap();
        assert!((dur.trim().parse::<f32>().unwrap() - 3.0).abs() < 0.001);
        let eas = rt.eval(r#"Bsengine.getTweenEasing("Box");"#).unwrap();
        assert_eq!(eas.trim().parse::<u32>().unwrap(), 2u32);
        let rep = rt.eval(r#"Bsengine.getTweenRepeat("Box");"#).unwrap();
        assert_eq!(rep.trim().parse::<u32>().unwrap(), 1u32);
        let el = rt.eval(r#"Bsengine.getTweenElapsed("Box");"#).unwrap();
        assert!((el.trim().parse::<f32>().unwrap() - 1.5).abs() < 0.001);
        let prog = rt.eval(r#"Bsengine.getTweenProgress("Box");"#).unwrap();
        assert!((prog.trim().parse::<f32>().unwrap() - 0.5).abs() < 0.001);
        let fin = rt.eval(r#"Bsengine.isTweenFinished("Box");"#).unwrap();
        assert_eq!(fin.trim(), "false");
        let rev = rt.eval(r#"Bsengine.isTweenReversed("Box");"#).unwrap();
        assert_eq!(rev.trim(), "true");
        let fin_unk = rt.eval(r#"Bsengine.isTweenFinished("Unknown");"#).unwrap();
        assert_eq!(fin_unk.trim(), "false");
        super::TWEEN_SNAPSHOT.with(|s| s.borrow_mut().remove("Box"));
    }

    #[test]
    fn test_tween_write_ops_queue_commands() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setTweenDuration("Box", 2.0);"#)
            .unwrap();
        rt.eval(r#"Bsengine.setTweenEasing("Box", 3);"#).unwrap();
        rt.eval(r#"Bsengine.setTweenRepeat("Box", 2);"#).unwrap();
        rt.eval(r#"Bsengine.setTweenElapsed("Box", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetTweenDuration { name, duration }
                if name == "Box" && (*duration - 2.0).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetTweenEasing { name, easing }
                if name == "Box" && *easing == 3
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetTweenRepeat { name, repeat }
                if name == "Box" && *repeat == 2
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetTweenElapsed { name, elapsed }
                if name == "Box" && (*elapsed - 0.5).abs() < 0.001
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_follow_read_ops() {
        super::FOLLOW_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                ("Camera".to_string(), 0.0, 2.0, -5.0, 10.0),
            );
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let target = rt.eval(r#"Bsengine.getFollowTarget("Player");"#).unwrap();
        assert_eq!(target.trim(), "Camera");
        let ox = rt.eval(r#"Bsengine.getFollowOffsetX("Player");"#).unwrap();
        assert!((ox.trim().parse::<f32>().unwrap() - 0.0).abs() < 0.001);
        let oy = rt.eval(r#"Bsengine.getFollowOffsetY("Player");"#).unwrap();
        assert!((oy.trim().parse::<f32>().unwrap() - 2.0).abs() < 0.001);
        let oz = rt.eval(r#"Bsengine.getFollowOffsetZ("Player");"#).unwrap();
        assert!((oz.trim().parse::<f32>().unwrap() - (-5.0)).abs() < 0.001);
        let sp = rt.eval(r#"Bsengine.getFollowSpeed("Player");"#).unwrap();
        assert!((sp.trim().parse::<f32>().unwrap() - 10.0).abs() < 0.001);
        super::FOLLOW_SNAPSHOT.with(|s| s.borrow_mut().clear());
    }

    #[test]
    fn test_follow_write_ops_queue_commands() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setFollowTarget("Player", "Camera");"#)
            .unwrap();
        rt.eval(r#"Bsengine.setFollowOffset("Player", 0.0, 2.0, -5.0);"#)
            .unwrap();
        rt.eval(r#"Bsengine.setFollowSpeed("Player", 10.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetFollowTarget { name, target }
                if name == "Player" && target == "Camera"
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetFollowOffset { name, x, y, z }
                if name == "Player" && x.abs() < 0.001 && (*y - 2.0).abs() < 0.001 && (*z - (-5.0)).abs() < 0.001
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetFollowSpeed { name, speed }
                if name == "Player" && (*speed - 10.0).abs() < 0.001
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_look_at_component_read_ops() {
        super::LOOK_AT_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Camera".to_string(), ("Enemy".to_string(), 0.0, 1.0, 0.0));
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let target = rt.eval(r#"Bsengine.getLookAtTarget("Camera");"#).unwrap();
        assert_eq!(target.trim(), "Enemy");
        let ux = rt.eval(r#"Bsengine.getLookAtUpX("Camera");"#).unwrap();
        assert!((ux.trim().parse::<f32>().unwrap() - 0.0).abs() < 0.001);
        let uy = rt.eval(r#"Bsengine.getLookAtUpY("Camera");"#).unwrap();
        assert!((uy.trim().parse::<f32>().unwrap() - 1.0).abs() < 0.001);
        let uz = rt.eval(r#"Bsengine.getLookAtUpZ("Camera");"#).unwrap();
        assert!((uz.trim().parse::<f32>().unwrap() - 0.0).abs() < 0.001);
        super::LOOK_AT_SNAPSHOT.with(|s| s.borrow_mut().clear());
    }

    #[test]
    fn test_look_at_component_write_ops_queue_commands() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setLookAtTarget("Camera", "Enemy");"#)
            .unwrap();
        rt.eval(r#"Bsengine.setLookAtUp("Camera", 0.0, 1.0, 0.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetLookAtTarget { name, target }
                if name == "Camera" && target == "Enemy"
            )));
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetLookAtUp { name, x, y, z }
                if name == "Camera" && x.abs() < 0.001 && (*y - 1.0).abs() < 0.001 && z.abs() < 0.001
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_network_id_read_ops() {
        // (id_str, authority_kind, peer_id_str)
        super::NETWORK_ID_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Server1".to_string(), ("42".to_string(), 0, String::new()));
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        assert_eq!(
            rt.eval(r#"Bsengine.getNetworkId("Server1")"#).unwrap(),
            "42"
        );
        assert_eq!(
            rt.eval(r#"Bsengine.getNetworkAuthority("Server1")"#)
                .unwrap(),
            "0"
        );
        assert_eq!(
            rt.eval(r#"Bsengine.getNetworkPeerId("Server1")"#).unwrap(),
            ""
        );
        assert_eq!(
            rt.eval(r#"Bsengine.isNetworkReplicated("Server1")"#)
                .unwrap(),
            "true"
        );
        super::NETWORK_ID_SNAPSHOT.with(|s| s.borrow_mut().clear());
    }

    #[test]
    fn network_start_server_enqueues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval("Bsengine.network.startServer(7777);").unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::NetworkStartServer { port } if *port == 7777)
            }));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn network_connect_enqueues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.network.connect("127.0.0.1", 7777);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::NetworkConnect { host, port }
                    if host == "127.0.0.1" && *port == 7777)
            }));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn network_disconnect_enqueues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval("Bsengine.network.disconnect();").unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf
                .iter()
                .any(|cmd| { matches!(cmd, super::ScriptCommand::NetworkDisconnect) }));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn network_state_read_ops() {
        super::NETWORK_STATE_SNAPSHOT.with(|s| *s.borrow_mut() = (true, true, 42, 3));
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        assert_eq!(
            rt.eval("Bsengine.network.isServer();").unwrap().trim(),
            "true"
        );
        assert_eq!(
            rt.eval("Bsengine.network.isConnected();").unwrap().trim(),
            "true"
        );
        assert_eq!(
            rt.eval("Bsengine.network.getMyPeerId();").unwrap().trim(),
            "42"
        );
        assert_eq!(
            rt.eval("Bsengine.network.getPeerCount();").unwrap().trim(),
            "3"
        );
        super::NETWORK_STATE_SNAPSHOT.with(|s| *s.borrow_mut() = (false, false, 0, 0));
    }

    #[test]
    fn test_ui_set_label_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"Bsengine.ui.setLabel("lbl", "Score: 0", 10, 20, 18);"#,
            "<test>",
        )
        .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetUiLabel { id, text, x, y, font_size }
                    if id == "lbl" && text == "Score: 0" && *x == 10.0 && *y == 20.0 && *font_size == 18.0
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_ui_set_button_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"Bsengine.ui.setButton("btn_play", "Play", 100, 200, 120, 40);"#,
            "<test>",
        )
        .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert!(buf.iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::SetUiButton { id, label, x, y, width, height }
                    if id == "btn_play" && label == "Play"
                    && *x == 100.0 && *y == 200.0 && *width == 120.0 && *height == 40.0
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_ui_clear_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(r#"Bsengine.ui.clear();"#, "<test>").unwrap();
        super::COMMAND_BUFFER.with(|c| {
            assert!(c
                .borrow()
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::ClearUiWidgets)));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_ui_is_clicked_returns_false_when_not_clicked() {
        super::UI_CLICKED_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt
            .eval(r#"String(Bsengine.ui.isClicked("btn_play"))"#)
            .unwrap();
        assert_eq!(result, "false");
    }

    #[test]
    fn test_ui_is_clicked_returns_true_when_snapshot_set() {
        super::UI_CLICKED_SNAPSHOT.with(|s| s.borrow_mut().push("btn_ok".to_string()));
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt
            .eval(r#"String(Bsengine.ui.isClicked("btn_ok"))"#)
            .unwrap();
        assert_eq!(result, "true");
        super::UI_CLICKED_SNAPSHOT.with(|s| s.borrow_mut().clear());
    }

    #[test]
    fn test_ui_remove_widget_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(r#"Bsengine.ui.remove("lbl");"#, "<test>")
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            assert!(c.borrow().iter().any(|cmd| matches!(
                cmd,
                super::ScriptCommand::RemoveUiWidget { id } if id == "lbl"
            )));
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_anim_set_trigger_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.asmSetTrigger("Hero", "jump");"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AsmSetTrigger { name, trigger }
                    if name == "Hero" && trigger == "jump")
            });
            assert!(found, "AsmSetTrigger not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_anim_set_float_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.asmSetFloat("Hero", "speed", 1.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AsmSetFloat { name, param, value }
                    if name == "Hero" && param == "speed" && (*value - 1.5).abs() < 1e-5)
            });
            assert!(found, "AsmSetFloat not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_anim_set_bool_queues_command() {
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.asmSetBool("Hero", "grounded", true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AsmSetBool { name, param, value }
                    if name == "Hero" && param == "grounded" && *value)
            });
            assert!(found, "AsmSetBool not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn test_anim_get_state_returns_snapshot() {
        super::ASM_STATE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Hero".to_string(), "run".to_string());
        });
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt.eval(r#"Bsengine.asmGetState("Hero")"#).unwrap();
        assert_eq!(result, "run");
        super::ASM_STATE_SNAPSHOT.with(|s| s.borrow_mut().clear());
    }
}
