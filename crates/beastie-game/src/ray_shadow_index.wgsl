struct Instance { world:mat4x4<f32>, inverse:mat4x4<f32>, material:vec4<f32>, tint:vec4<f32>, root:u32, transmission:f32, pad1:u32, pad2:u32 }
struct VisibilityParams { clip_from_world:mat4x4<f32> }
@group(0) @binding(0) var<storage,read> positions:array<vec4<f32>>;
@group(0) @binding(1) var<storage,read> instances:array<Instance>;
@group(0) @binding(2) var<uniform> params:VisibilityParams;
@vertex fn shadow_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->@builtin(position) vec4<f32> {
    return params.clip_from_world*instances[instance].world*positions[vertex];
}
