// Raster visibility reuses the ray scene's packed triangle positions and instances.
struct Triangle { a:vec3<f32>, surface:u32, e1:vec4<f32>, e2:vec4<f32> }
struct Instance { world:mat4x4<f32>, inverse:mat4x4<f32>, material:vec4<f32>, tint:vec4<f32>, root:u32, transmission:f32, pad1:u32, pad2:u32 }
struct VisibilityParams { clip_from_world:mat4x4<f32> }
@group(0) @binding(0) var<storage,read> triangles:array<Triangle>;
@group(0) @binding(1) var<storage,read> instances:array<Instance>;
@group(0) @binding(2) var<uniform> params:VisibilityParams;
struct Vertex {
 @builtin(position) position:vec4<f32>,
 @location(0) @interpolate(flat) ids:vec2<u32>,
}
@vertex fn visibility_vertex(@builtin(vertex_index) vertex:u32, @builtin(instance_index) instance:u32) -> Vertex {
 let index=vertex/3u; let corner=vertex%3u; let tri=triangles[index];
 var p=tri.a.xyz;
 if(corner==1u) { p+=tri.e1.xyz; }
 if(corner==2u) { p+=tri.e2.xyz; }
 return Vertex(params.clip_from_world*instances[instance].world*vec4(p,1.0), vec2(index,instance+1u));
}
@fragment fn visibility_fragment(vertex:Vertex) -> @location(0) vec2<u32> { return vertex.ids; }

@fragment fn visibility_fragment_packed(vertex:Vertex) -> @location(0) u32 { return (vertex.ids.x<<8u)|vertex.ids.y; }
