// --- Value types -------------------------------------------------------
//
// Modelled on Unity's Vector3/Quaternion: same names, same static/instance
// split, so knowledge transfers. Three things differ, and only because JS
// forces them:
//
//   1. No operator overloading. `a + b` is `a.add(b)`; `q * v` is
//      `q.rotate(v)`; `q * p` is `q.mul(p)`.
//   2. No `ref`/`out`. The Unity methods that hand back extra values return
//      objects instead: smoothDamp -> {value, velocity},
//      orthoNormalize -> {normal, tangent}, toAngleAxis -> {angle, axis}.
//   3. **These are immutable.** Unity's Vector3 is a struct, so its Set() and
//      instance Normalize() mutate a copy and are safe. A JS object is a
//      reference, so the same methods would let one script corrupt a vector
//      another script is still holding. Every operation returns a new
//      instance, and Unity's mutating methods are deliberately absent.
//
// `function` rather than `class`, and hung off `Bsengine` rather than declared
// at top level, for the same reason the global is a `var`: the prelude is
// re-executed in the SAME V8 isolate on scene reload, and a top-level `class`
// throws "Identifier has already been declared" on the second run.

function _V3(x, y, z) {
    this.x = x;
    this.y = y;
    this.z = z;
}
_V3.prototype = {
    get magnitude() { return Math.sqrt(this.x * this.x + this.y * this.y + this.z * this.z); },
    get sqrMagnitude() { return this.x * this.x + this.y * this.y + this.z * this.z; },
    get normalized() {
        const m = this.magnitude;
        // Unity returns zero rather than NaN for a zero-length vector. A NaN
        // that escapes into a Transform is invisible until something
        // downstream renders nothing at all.
        return m > 1e-9 ? new _V3(this.x / m, this.y / m, this.z / m) : new _V3(0, 0, 0);
    },
    add(v) { return new _V3(this.x + v.x, this.y + v.y, this.z + v.z); },
    sub(v) { return new _V3(this.x - v.x, this.y - v.y, this.z - v.z); },
    mul(s) { return new _V3(this.x * s, this.y * s, this.z * s); },
    div(s) { return new _V3(this.x / s, this.y / s, this.z / s); },
    neg()  { return new _V3(-this.x, -this.y, -this.z); },
    equals(v, tolerance) {
        const t = tolerance === undefined ? 1e-5 : tolerance;
        return Math.abs(this.x - v.x) <= t
            && Math.abs(this.y - v.y) <= t
            && Math.abs(this.z - v.z) <= t;
    },
    clone() { return new _V3(this.x, this.y, this.z); },
    toString() { return "(" + this.x + ", " + this.y + ", " + this.z + ")"; },
};

// `var`, not `const`: scene reload re-runs this bootstrap in the SAME V8
// isolate/global scope (see handle_scene_load in bsengine-runtime) rather
// than spinning up a new isolate. `const`/`let` at top level would throw
// "Identifier 'Bsengine' has already been declared" on the second run;
// `var` (and plain reassignment) is redeclaration-safe.
function _Q(x, y, z, w) {
    this.x = x;
    this.y = y;
    this.z = z;
    this.w = w;
}
_Q.prototype = {
    get normalized() {
        const m = Math.sqrt(this.x*this.x + this.y*this.y + this.z*this.z + this.w*this.w);
        return m > 1e-9 ? new _Q(this.x/m, this.y/m, this.z/m, this.w/m) : new _Q(0, 0, 0, 1);
    },
    // Degrees, and the YXZ order the engine itself composes -- see
    // `ScriptCommand::SetRotationEuler`, which calls
    // `Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll)`. Disagreeing with it
    // would make a script that reads these and writes them back point the
    // entity somewhere else. Derived as the inverse of R = Ry*Rx*Rz.
    get eulerAngles() {
        const x = this.x, y = this.y, z = this.z, w = this.w;
        const sinPitch = Math.max(-1, Math.min(1, 2 * (w * x - y * z)));
        const pitch = Math.asin(sinPitch);
        const yaw   = Math.atan2(2 * (w * y + x * z), 1 - 2 * (x * x + y * y));
        const roll  = Math.atan2(2 * (w * z + x * y), 1 - 2 * (x * x + z * z));
        const d = 180 / Math.PI;
        return new _V3(pitch * d, yaw * d, roll * d);
    },
    // Unity's `a * b`: b is applied first, then a.
    mul(q) {
        return new _Q(
            this.w*q.x + this.x*q.w + this.y*q.z - this.z*q.y,
            this.w*q.y - this.x*q.z + this.y*q.w + this.z*q.x,
            this.w*q.z + this.x*q.y - this.y*q.x + this.z*q.w,
            this.w*q.w - this.x*q.x - this.y*q.y - this.z*q.z);
    },
    // Unity's `q * v`.
    rotate(v) {
        const u = new _V3(this.x, this.y, this.z);
        const s = this.w;
        return u.mul(2 * Bsengine.Vec3.dot(u, v))
            .add(v.mul(s * s - u.sqrMagnitude))
            .add(Bsengine.Vec3.cross(u, v).mul(2 * s));
    },
    toAngleAxis() {
        const q = this.normalized;
        const angle = 2 * Math.acos(Math.max(-1, Math.min(1, q.w)));
        const s = Math.sqrt(Math.max(0, 1 - q.w * q.w));
        // Near zero rotation the axis is arbitrary; Unity returns +X.
        const axis = s < 1e-6 ? new _V3(1, 0, 0) : new _V3(q.x / s, q.y / s, q.z / s);
        return { angle: angle * 180 / Math.PI, axis };
    },
    equals(q, tolerance) {
        const t = tolerance === undefined ? 1e-5 : tolerance;
        return Math.abs(this.x - q.x) <= t && Math.abs(this.y - q.y) <= t
            && Math.abs(this.z - q.z) <= t && Math.abs(this.w - q.w) <= t;
    },
    clone() { return new _Q(this.x, this.y, this.z, this.w); },
    toString() { return "(" + this.x + ", " + this.y + ", " + this.z + ", " + this.w + ")"; },
};

// The op layer speaks flat records and bare arrays; these two functions are
// the only places that translation happens.
function _tf(t) {
    return {
        position: new _V3(t.x, t.y, t.z),
        rotation: new _Q(t.rx, t.ry, t.rz, t.rw),
        scale:    new _V3(t.sx, t.sy, t.sz),
    };
}
function _v3OrNull(a) {
    return a ? new _V3(a[0], a[1], a[2]) : null;
}

// A setter takes either loose scalars or one vector, because the getter it
// pairs with returns a vector: `setPosition(n, getPosition(m))` has to work or
// the pair is lying. Judged by shape rather than `instanceof` -- a scene
// reload replaces the prototype, and a plain {x,y,z} literal is a perfectly
// good argument.
function _xyz(a, b, c) {
    return (a !== null && typeof a === "object") ? [a.x, a.y, a.z] : [a, b, c];
}
function _xyzw(a, b, c, d) {
    return (a !== null && typeof a === "object") ? [a.x, a.y, a.z, a.w] : [a, b, c, d];
}

var Bsengine = {
    Vec3: _V3,
    Quat: _Q,
    quat: (x, y, z, w) => new _Q(x, y, z, w),
    vec3: (x, y, z) => new _V3(x, y, z),
    log:            (msg)                  => Deno.core.ops.bsengine_log(msg),
    version:        ()                     => Deno.core.ops.bsengine_version(),
    getTransform:      (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? _tf(t) : null; },
    getPosition:       (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? new _V3(t.x, t.y, t.z) : null; },
    getRotation:       (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? new _Q(t.rx, t.ry, t.rz, t.rw) : null; },
    getScale:          (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? new _V3(t.sx, t.sy, t.sz) : null; },
    getForwardVector:  (name)                 => _v3OrNull(Deno.core.ops.bsengine_get_forward_vector(name)),
    getRightVector:    (name)                 => _v3OrNull(Deno.core.ops.bsengine_get_right_vector(name)),
    getUpVector:       (name)                 => _v3OrNull(Deno.core.ops.bsengine_get_up_vector(name)),
    distanceTo:        (nameA, nameB)         => Deno.core.ops.bsengine_distance_to(nameA, nameB),
    distanceToPoint:   (name, x, y, z)       => Deno.core.ops.bsengine_distance_to_point(name, x, y, z),
    getWorldTransform: (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? _tf(t) : null; },
    getWorldPosition:  (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? new _V3(t.x, t.y, t.z) : null; },
    getWorldRotation:  (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? new _Q(t.rx, t.ry, t.rz, t.rw) : null; },
    getWorldScale:     (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? new _V3(t.sx, t.sy, t.sz) : null; },
    setPosition:    (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_position(name, x, y, z); },
    // Takes exactly what getTransform returns, so `setTransform(b,
    // getTransform(a))` copies one entity onto another. Until this existed,
    // `setTransform` set only the position while `getTransform` returned all
    // three -- the asymmetry that made someone write a `setPosition` that was
    // not there, in games/net-2p-demo, for as long as it existed.
    setTransform:   (name, t) => Deno.core.ops.bsengine_set_transform(
        name,
        t.position.x, t.position.y, t.position.z,
        t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w,
        t.scale.x, t.scale.y, t.scale.z),
    setRotation: (name, a, b, c, d) => { const [x, y, z, w] = _xyzw(a, b, c, d); Deno.core.ops.bsengine_set_rotation(name, x, y, z, w); },
    setRotationEuler: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_rotation_euler(name, x, y, z); },
    setScale: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_scale(name, x, y, z); },
    addPosition: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_position(name, x, y, z); },
    addPositionLocal: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_position_local(name, x, y, z); },
    rotateBy: (name, a, b, c, d) => { const [x, y, z, w] = _xyzw(a, b, c, d); Deno.core.ops.bsengine_rotate_by(name, x, y, z, w); },
    rotateAroundAxis:  (name, ax, ay, az, deg)  => Deno.core.ops.bsengine_rotate_around_axis(name, ax, ay, az, deg),
    addRotationEuler: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_rotation_euler(name, x, y, z); },
    addScale: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_scale(name, x, y, z); },
    multiplyScale: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_multiply_scale(name, x, y, z); },
    isKeyPressed:   (key)                  => Deno.core.ops.bsengine_is_key_pressed(key),
    isKeyDown:      (key)                  => Deno.core.ops.bsengine_is_key_down(key),
    isKeyUp:        (key)                  => Deno.core.ops.bsengine_is_key_up(key),
    pause:          ()                     => Deno.core.ops.bsengine_pause(),
    resume:         ()                     => Deno.core.ops.bsengine_resume(),
    isPaused:       ()                     => Deno.core.ops.bsengine_is_paused(),
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
    instantiatePrefab: (params)            => Deno.core.ops.bsengine_instantiate_prefab(params),
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
    setDirectionalLightDirection: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_directional_light_direction(name, x, y, z); },
    setCameraFov:   (name, deg)            => Deno.core.ops.bsengine_set_camera_fov(name, deg),
    setCameraNear:  (name, value)          => Deno.core.ops.bsengine_set_camera_near(name, value),
    setCameraFar:   (name, value)          => Deno.core.ops.bsengine_set_camera_far(name, value),
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
    moveEntity:             (name, dx, dy, dz) => Deno.core.ops.bsengine_move_entity(name, dx, dy, dz),
    quit:                   ()              => Deno.core.ops.bsengine_quit(),
    restoreShield:          (name, amount)  => Deno.core.ops.bsengine_restore_shield(name, amount),
    setMaxShield:           (name, value)   => Deno.core.ops.bsengine_set_max_shield(name, value),
    getShield:              (name)          => Deno.core.ops.bsengine_get_shield(name),
    getMaxShield:           (name)          => Deno.core.ops.bsengine_get_max_shield(name),
    getShieldFraction:      (name)          => Deno.core.ops.bsengine_get_shield_fraction(name),
    isShieldDepleted:       (name)          => Deno.core.ops.bsengine_is_shield_depleted(name),
    setSaveField:           (name, key, value) => Deno.core.ops.bsengine_set_save_field(name, key, String(value)),
    getSaveField:           (name, key)        => Deno.core.ops.bsengine_get_save_field(name, key),
    resetTimer:             (name)          => Deno.core.ops.bsengine_reset_timer(name),
    getTimerElapsed:        (name)          => Deno.core.ops.bsengine_get_timer_elapsed(name),
    getTimerDuration:       (name)          => Deno.core.ops.bsengine_get_timer_duration(name),
    getTimerFraction:       (name)          => Deno.core.ops.bsengine_get_timer_fraction(name),
    isTimerFinished:        (name)          => Deno.core.ops.bsengine_is_timer_finished(name),
    isTimerJustFinished:    (name)          => Deno.core.ops.bsengine_is_timer_just_finished(name),
    setNavDestination: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_nav_destination(name, x, y, z); },
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
    setFollowOffset: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_follow_offset(name, x, y, z); },
    setFollowSpeed: (name, speed)          => Deno.core.ops.bsengine_set_follow_speed(name, speed),
    getFollowTarget:(name)                 => JSON.parse(Deno.core.ops.bsengine_get_follow_target(name)),
    getFollowOffset:(name)                => _v3OrNull(Deno.core.ops.bsengine_get_follow_offset(name)),
    getFollowSpeed: (name)                 => Deno.core.ops.bsengine_get_follow_speed(name),
    setLookAtTarget:(name, target)         => Deno.core.ops.bsengine_set_look_at_target(name, target),
    setLookAtUp: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_look_at_up(name, x, y, z); },
    getLookAtTarget:(name)                 => JSON.parse(Deno.core.ops.bsengine_get_look_at_target(name)),
    getLookAtUp:    (name)                 => _v3OrNull(Deno.core.ops.bsengine_get_look_at_up(name)),
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






































    lookAt: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_look_at(name, x, y, z); },

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
    getVelocity:      (name)    => _v3OrNull(Deno.core.ops.bsengine_get_velocity(name)),
    getLinearSpeed:   (name)    => { const s = Deno.core.ops.bsengine_get_linear_speed(name); return s !== null && s !== undefined ? s[0] : -1; },
    addImpulse: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_impulse(name, x, y, z); },
    applyImpulseAtPoint: (name, fx, fy, fz, px, py, pz) => Deno.core.ops.bsengine_apply_impulse_at_point(name, fx, fy, fz, px, py, pz),
    addForce: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_force(name, x, y, z); },
    addForceAtPoint:  (name, fx, fy, fz, px, py, pz) => Deno.core.ops.bsengine_add_force_at_point(name, fx, fy, fz, px, py, pz),
    // Discards force/torque added earlier in this same frame, before the
    // physics step applies it. Forces last exactly one step, so this is only
    // for "something already pushed this frame and now we want it not to" —
    // a teleport or a freeze. setVelocity(0,0,0) alone is enough otherwise.
    resetForces:      (name) => Deno.core.ops.bsengine_reset_forces(name),
    // Emits one burst of `burst_count` particles from the entity's emitter, at
    // wherever that entity currently is -- so a hit effect moves the emitter to
    // the impact point first.
    burstParticles:   (name) => Deno.core.ops.bsengine_burst_particles(name),
    setVelocity: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_velocity(name, x, y, z); },
    getGravity:           ()                     => Deno.core.ops.bsengine_get_gravity(),
    setGravity:           (magnitude)             => Deno.core.ops.bsengine_set_gravity(magnitude),
    getAngularVelocity:   (name)                  => _v3OrNull(Deno.core.ops.bsengine_get_angular_velocity(name)),
    setAngularVelocity: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_set_angular_velocity(name, x, y, z); },
    addVelocity: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_velocity(name, x, y, z); },
    addAngularVelocity: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_angular_velocity(name, x, y, z); },
    addAngularImpulse: (name, a, b, c) => { const [x, y, z] = _xyz(a, b, c); Deno.core.ops.bsengine_add_angular_impulse(name, x, y, z); },
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
    playSound3D:    (entity, path, opts) => {
        const v = (opts && opts.volume !== undefined) ? opts.volume : 1.0;
        const l = (opts && opts.loop) ? true : false;
        return Deno.core.ops.bsengine_play_sound_3d(entity, path, v, l);
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
    // What became of an asset load, as "loaded" | "loading" |
    // "failed: <reason>" | "unknown". "unknown" means nothing ever asked for
    // that path -- deliberately *not* the same answer as a failure, so a
    // typo'd path and a genuinely broken one are told apart:
    //   var s = Bsengine.getAssetStatus("assets/sounds/hit.wav");
    //   if (s.startsWith("failed:")) { ... }   // broken, and s says why
    //   else if (s === "unknown")    { ... }   // nobody ever requested it
    // `path` is project-relative and forward-slashed -- the same string you
    // pass to playSound/setShader/loadScene, and the same form a scene's
    // "gltf:" field uses. The engine's own fully-qualified key (the project
    // directory this host was started with, then that path) also works, but
    // nothing tells a script what that prefix is, so prefer the short form.
    // String() for the same reason setHudText below coerces: deno_core turns a
    // non-string argument into "" without complaint, which would silently read
    // as "unknown" for every mistyped call.
    getAssetStatus:       (path)        => Deno.core.ops.bsengine_get_asset_status(String(path)),
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
        setLabel:       (id, text, x, y, fontSize)          => Deno.core.ops.bsengine_ui_set_label(id, String(text), x, y, fontSize ?? 20),
        setButton:      (id, label, x, y, width, height)    => Deno.core.ops.bsengine_ui_set_button(id, label, x, y, width, height),
        setPanel:       (id, title, x, y, width, height)    => Deno.core.ops.bsengine_ui_set_panel(id, title ?? '', x, y, width, height),
        setTextInput:   (id, hint, x, y, width)             => Deno.core.ops.bsengine_ui_set_text_input(id, hint ?? '', x, y, width),
        setProgressBar: (id, x, y, width, height, fraction) => Deno.core.ops.bsengine_ui_set_progress_bar(id, x, y, width, height, fraction),
        remove:         (id)                                => Deno.core.ops.bsengine_ui_remove_widget(id),
        clear:          ()                                  => Deno.core.ops.bsengine_ui_clear(),
        isClicked:      (id)                                => Deno.core.ops.bsengine_ui_is_clicked(id),
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

// --- Vec3 statics ------------------------------------------------------
//
// Shared instances, which is safe only because the type is immutable: with a
// mutating method, `Bsengine.Vec3.up.x = 5` would poison every later reader.
Bsengine.Vec3.zero    = new _V3(0, 0, 0);
Bsengine.Vec3.one     = new _V3(1, 1, 1);
Bsengine.Vec3.up      = new _V3(0, 1, 0);
Bsengine.Vec3.down    = new _V3(0, -1, 0);
Bsengine.Vec3.left    = new _V3(-1, 0, 0);
Bsengine.Vec3.right   = new _V3(1, 0, 0);
Bsengine.Vec3.forward = new _V3(0, 0, 1);
Bsengine.Vec3.back    = new _V3(0, 0, -1);
Bsengine.Vec3.positiveInfinity = new _V3(Infinity, Infinity, Infinity);
Bsengine.Vec3.negativeInfinity = new _V3(-Infinity, -Infinity, -Infinity);

// --- Vec3 geometry -----------------------------------------------------
Bsengine.Vec3.dot   = (a, b) => a.x * b.x + a.y * b.y + a.z * b.z;
Bsengine.Vec3.cross = (a, b) => new _V3(
    a.y * b.z - a.z * b.y,
    a.z * b.x - a.x * b.z,
    a.x * b.y - a.y * b.x);
Bsengine.Vec3.distance  = (a, b) => a.sub(b).magnitude;
Bsengine.Vec3.normalize = (v) => v.normalized;
Bsengine.Vec3.scale = (a, b) => new _V3(a.x * b.x, a.y * b.y, a.z * b.z);
Bsengine.Vec3.min   = (a, b) => new _V3(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.min(a.z, b.z));
Bsengine.Vec3.max   = (a, b) => new _V3(Math.max(a.x, b.x), Math.max(a.y, b.y), Math.max(a.z, b.z));

// Degrees, 0..180 -- Unity's convention. Radians here would be a silent
// factor of ~57 in every script that turns to face something.
Bsengine.Vec3.angle = (a, b) => {
    const d = a.magnitude * b.magnitude;
    if (d < 1e-9) return 0;
    // Clamped because floating point can push the quotient just past +/-1,
    // where Math.acos returns NaN -- and a NaN angle becomes a NaN rotation
    // and an entity that renders nowhere.
    const c = Math.max(-1, Math.min(1, Bsengine.Vec3.dot(a, b) / d));
    return Math.acos(c) * 180 / Math.PI;
};
Bsengine.Vec3.signedAngle = (a, b, axis) => {
    const unsigned = Bsengine.Vec3.angle(a, b);
    const sign = Math.sign(Bsengine.Vec3.dot(axis, Bsengine.Vec3.cross(a, b)));
    return unsigned * (sign === 0 ? 1 : sign);
};

Bsengine.Vec3.clampMagnitude = (v, maxLength) =>
    v.sqrMagnitude > maxLength * maxLength ? v.normalized.mul(maxLength) : v.clone();
Bsengine.Vec3.project = (v, onNormal) => {
    const d = onNormal.sqrMagnitude;
    return d < 1e-9 ? new _V3(0, 0, 0) : onNormal.mul(Bsengine.Vec3.dot(v, onNormal) / d);
};
Bsengine.Vec3.projectOnPlane = (v, planeNormal) => v.sub(Bsengine.Vec3.project(v, planeNormal));
Bsengine.Vec3.reflect = (inDirection, inNormal) =>
    inDirection.sub(inNormal.mul(2 * Bsengine.Vec3.dot(inDirection, inNormal)));

// --- Vec3 interpolation ------------------------------------------------
Bsengine.Vec3.lerpUnclamped = (a, b, t) => a.add(b.sub(a).mul(t));
Bsengine.Vec3.lerp = (a, b, t) =>
    Bsengine.Vec3.lerpUnclamped(a, b, Math.max(0, Math.min(1, t)));

Bsengine.Vec3.slerpUnclamped = (a, b, t) => {
    const ma = a.magnitude, mb = b.magnitude;
    // With a zero-length end the arc is undefined; the straight line is what
    // it degenerates to.
    if (ma < 1e-9 || mb < 1e-9) return Bsengine.Vec3.lerpUnclamped(a, b, t);
    const na = a.div(ma), nb = b.div(mb);
    const c = Math.max(-1, Math.min(1, Bsengine.Vec3.dot(na, nb)));
    const theta = Math.acos(c);
    if (theta < 1e-6) return Bsengine.Vec3.lerpUnclamped(a, b, t);
    const len = ma + (mb - ma) * t;
    const s = Math.sin(theta);
    return na.mul(Math.sin((1 - t) * theta) / s)
             .add(nb.mul(Math.sin(t * theta) / s))
             .mul(len);
};
Bsengine.Vec3.slerp = (a, b, t) =>
    Bsengine.Vec3.slerpUnclamped(a, b, Math.max(0, Math.min(1, t)));

// Never overshoots -- the property that lets a caller use this as a per-frame
// step without an epsilon check of its own.
Bsengine.Vec3.moveTowards = (current, target, maxDistanceDelta) => {
    const d = target.sub(current);
    const m = d.magnitude;
    if (m <= maxDistanceDelta || m < 1e-9) return target.clone();
    return current.add(d.div(m).mul(maxDistanceDelta));
};

Bsengine.Vec3.rotateTowards = (current, target, maxRadiansDelta, maxMagnitudeDelta) => {
    const len = Bsengine.Vec3.moveTowards(
        new _V3(current.magnitude, 0, 0),
        new _V3(target.magnitude, 0, 0),
        maxMagnitudeDelta).x;
    const angleRad = Bsengine.Vec3.angle(current, target) * Math.PI / 180;
    if (angleRad < 1e-6) return target.normalized.mul(len);
    const t = Math.min(1, maxRadiansDelta / angleRad);
    return Bsengine.Vec3.slerp(current, target, t).normalized.mul(len);
};

// Unity takes `ref currentVelocity`; JS has no ref, so the new velocity comes
// back beside the value and the CALLER threads it between frames. Dropping it
// eases on the first frame and jitters after.
Bsengine.Vec3.smoothDamp = (current, target, currentVelocity, smoothTime, maxSpeed, deltaTime) => {
    // The critically damped spring Unity uses (Game Programming Gems 4).
    const st = Math.max(0.0001, smoothTime);
    const omega = 2 / st;
    const x = omega * deltaTime;
    const exp = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current.sub(target);
    const maxChange = maxSpeed * st;
    if (isFinite(maxChange) && change.sqrMagnitude > maxChange * maxChange) {
        change = change.normalized.mul(maxChange);
    }
    const dest = current.sub(change);
    const temp = currentVelocity.add(change.mul(omega)).mul(deltaTime);
    let newVel = currentVelocity.sub(temp.mul(omega)).mul(exp);
    let output = dest.add(change.add(temp).mul(exp));
    // Do not step past the target.
    if (Bsengine.Vec3.dot(target.sub(current), output.sub(target)) > 0) {
        output = target.clone();
        newVel = output.sub(target).div(deltaTime);
    }
    return { value: output, velocity: newVel };
};

// Unity mutates two `ref` parameters; both come back here instead.
Bsengine.Vec3.orthoNormalize = (normal, tangent) => {
    const n = normal.normalized;
    return { normal: n, tangent: tangent.sub(Bsengine.Vec3.project(tangent, n)).normalized };
};

// --- Quat statics ------------------------------------------------------
Bsengine.Quat.identity = new _Q(0, 0, 0, 1);

Bsengine.Quat.angleAxis = (angleDeg, axis) => {
    const a = axis.normalized;
    const h = angleDeg * Math.PI / 360;
    const s = Math.sin(h);
    return new _Q(a.x * s, a.y * s, a.z * s, Math.cos(h));
};

// Composed Y, then X, then Z -- the engine's own order, spelled out rather
// than as a closed-form so it cannot silently drift from
// `Quat::from_euler(EulerRot::YXZ, ...)`.
Bsengine.Quat.euler = (pitch, yaw, roll) =>
    Bsengine.Quat.angleAxis(yaw, Bsengine.Vec3.up)
        .mul(Bsengine.Quat.angleAxis(pitch, Bsengine.Vec3.right))
        .mul(Bsengine.Quat.angleAxis(roll, Bsengine.Vec3.forward));

Bsengine.Quat.dot = (a, b) => a.x*b.x + a.y*b.y + a.z*b.z + a.w*b.w;
Bsengine.Quat.normalize = (q) => q.normalized;
// The conjugate of a unit quaternion is its inverse.
Bsengine.Quat.inverse = (q) => {
    const n = q.normalized;
    return new _Q(-n.x, -n.y, -n.z, n.w);
};
Bsengine.Quat.angle = (a, b) => {
    const d = Math.abs(Math.max(-1, Math.min(1,
        Bsengine.Quat.dot(a.normalized, b.normalized))));
    return 2 * Math.acos(d) * 180 / Math.PI;
};

// --- Quat: facing and interpolation ------------------------------------
//
// The rotation whose forward is `forward` and whose up is as close to `up` as
// that allows. Returns identity for a degenerate direction rather than NaN --
// a chase script calls this on the frame it reaches its target, and a NaN
// rotation is an entity that renders nowhere.
Bsengine.Quat.lookRotation = (forward, up) => {
    const u = up === undefined ? Bsengine.Vec3.up : up;
    const f = forward.normalized;
    if (f.sqrMagnitude < 1e-9) return Bsengine.Quat.identity;
    let r = Bsengine.Vec3.cross(u, f);
    if (r.sqrMagnitude < 1e-9) {
        // forward is parallel to up; any perpendicular will do.
        r = Bsengine.Vec3.cross(
            Math.abs(f.y) > 0.99 ? Bsengine.Vec3.right : Bsengine.Vec3.up, f);
    }
    r = r.normalized;
    const realUp = Bsengine.Vec3.cross(f, r);
    // Rotation matrix with columns (right, up, forward) -> quaternion.
    const m00 = r.x, m01 = realUp.x, m02 = f.x;
    const m10 = r.y, m11 = realUp.y, m12 = f.y;
    const m20 = r.z, m21 = realUp.z, m22 = f.z;
    const tr = m00 + m11 + m22;
    if (tr > 0) {
        const s = Math.sqrt(tr + 1) * 2;
        return new _Q((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, s / 4);
    }
    if (m00 > m11 && m00 > m22) {
        const s = Math.sqrt(1 + m00 - m11 - m22) * 2;
        return new _Q(s / 4, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s);
    }
    if (m11 > m22) {
        const s = Math.sqrt(1 + m11 - m00 - m22) * 2;
        return new _Q((m01 + m10) / s, s / 4, (m12 + m21) / s, (m02 - m20) / s);
    }
    const s = Math.sqrt(1 + m22 - m00 - m11) * 2;
    return new _Q((m02 + m20) / s, (m12 + m21) / s, s / 4, (m10 - m01) / s);
};

Bsengine.Quat.fromToRotation = (from, to) => {
    const f = from.normalized, t = to.normalized;
    const d = Bsengine.Vec3.dot(f, t);
    if (d > 1 - 1e-9) return Bsengine.Quat.identity;
    if (d < -1 + 1e-9) {
        // Opposite: half a turn about any perpendicular axis.
        let axis = Bsengine.Vec3.cross(Bsengine.Vec3.right, f);
        if (axis.sqrMagnitude < 1e-9) axis = Bsengine.Vec3.cross(Bsengine.Vec3.up, f);
        return Bsengine.Quat.angleAxis(180, axis.normalized);
    }
    const c = Bsengine.Vec3.cross(f, t);
    return new _Q(c.x, c.y, c.z, 1 + d).normalized;
};

// Sign-flipped when the dot product is negative so the interpolation takes the
// short way round; without it, rotations more than 180 degrees apart spin the
// long way -- a full turn where there should be a twitch.
Bsengine.Quat.lerpUnclamped = (a, b, t) => {
    const s = Bsengine.Quat.dot(a, b) < 0 ? -1 : 1;
    return new _Q(
        a.x + (b.x * s - a.x) * t,
        a.y + (b.y * s - a.y) * t,
        a.z + (b.z * s - a.z) * t,
        a.w + (b.w * s - a.w) * t).normalized;
};
Bsengine.Quat.lerp = (a, b, t) =>
    Bsengine.Quat.lerpUnclamped(a, b, Math.max(0, Math.min(1, t)));

Bsengine.Quat.slerpUnclamped = (a, b, t) => {
    let cos = Bsengine.Quat.dot(a.normalized, b.normalized);
    let end = b;
    if (cos < 0) { cos = -cos; end = new _Q(-b.x, -b.y, -b.z, -b.w); }
    if (cos > 1 - 1e-6) return Bsengine.Quat.lerpUnclamped(a, end, t);
    const theta = Math.acos(Math.max(-1, Math.min(1, cos)));
    const s = Math.sin(theta);
    const wa = Math.sin((1 - t) * theta) / s;
    const wb = Math.sin(t * theta) / s;
    return new _Q(
        a.x * wa + end.x * wb,
        a.y * wa + end.y * wb,
        a.z * wa + end.z * wb,
        a.w * wa + end.w * wb).normalized;
};
Bsengine.Quat.slerp = (a, b, t) =>
    Bsengine.Quat.slerpUnclamped(a, b, Math.max(0, Math.min(1, t)));

Bsengine.Quat.rotateTowards = (from, to, maxDegreesDelta) => {
    const total = Bsengine.Quat.angle(from, to);
    if (total < 1e-6) return to.clone();
    // No clamp on the fraction: `slerp` clamps t to [0,1] itself, so one here
    // would be a line whose removal changes nothing observable.
    return Bsengine.Quat.slerp(from, to, maxDegreesDelta / total);
};
