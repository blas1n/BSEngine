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
    // Discards force/torque added earlier in this same frame, before the
    // physics step applies it. Forces last exactly one step, so this is only
    // for "something already pushed this frame and now we want it not to" —
    // a teleport or a freeze. setVelocity(0,0,0) alone is enough otherwise.
    resetForces:      (name) => Deno.core.ops.bsengine_reset_forces(name),
    // Emits one burst of `burst_count` particles from the entity's emitter, at
    // wherever that entity currently is -- so a hit effect moves the emitter to
    // the impact point first.
    burstParticles:   (name) => Deno.core.ops.bsengine_burst_particles(name),
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
