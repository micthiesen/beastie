// Geometry, letters and interface all intersect in this same ordinary-compute path.
struct Triangle { a:vec4<f32>, b:vec4<f32>, c:vec4<f32>, n0:vec4<f32>, n1:vec4<f32>, n2:vec4<f32>, c0:vec4<f32>, c1:vec4<f32>, c2:vec4<f32> }
struct Node { lo:vec3<f32>, first:u32, hi:vec3<f32>, count:u32 }
struct Instance { world:mat4x4<f32>, inverse:mat4x4<f32>, material:vec4<f32>, tint:vec4<f32>, root:u32, transmission:f32, pad1:u32, pad2:u32 }
struct Params { world_from_clip:mat4x4<f32>, size:vec4<u32> }
@group(0) @binding(0) var<storage,read> triangles:array<Triangle>;
@group(0) @binding(1) var<storage,read> nodes:array<Node>;
@group(0) @binding(2) var<storage,read> instances:array<Instance>;
@group(0) @binding(3) var<storage,read> tlas:array<Node>;
@group(0) @binding(4) var<uniform> params:Params;
@group(0) @binding(5) var output:texture_storage_2d<rgba16float,write>;
struct Hit { t:f32, u:f32, v:f32, triangle:u32, instance:u32 }
fn box_hit(o:vec3<f32>, d:vec3<f32>, lo:vec3<f32>, hi:vec3<f32>, limit:f32) -> f32 {
    // Return the entry distance, or limit for a miss. Explicit parallel slabs
    // avoid 0 * infinity and NaNs on exact voxel boundaries.
    var near=0.0001; var far=limit;
    for(var axis=0u;axis<3u;axis++) {
        if(abs(d[axis])<1e-10) {
            if(o[axis]<lo[axis] || o[axis]>hi[axis]) { return limit; }
        } else {
            let a=(lo[axis]-o[axis])/d[axis]; let b=(hi[axis]-o[axis])/d[axis];
            near=max(near,min(a,b)); far=min(far,max(a,b));
            if(far<near) { return limit; }
        }
    }
    return select(limit,near,far>=near);
}
fn triangle_hit(o:vec3<f32>, d:vec3<f32>, index:u32, closest:Hit) -> Hit {
    let tri=triangles[index]; let e1=tri.b.xyz-tri.a.xyz; let e2=tri.c.xyz-tri.a.xyz;
    let p=cross(d,e2); let det=dot(e1,p);
    if(abs(det)<1e-10) { return closest; }
    let inverse_det=1.0/det;
    let q0=o-tri.a.xyz; let u=dot(q0,p)*inverse_det;
    if(u < -0.000001 || u>1.000001) { return closest; }
    let q=cross(q0,e1); let v=dot(d,q)*inverse_det;
    if(v < -0.000001 || u+v>1.000001) { return closest; }
    let t=dot(e2,q)*inverse_det;
    if(t<=0.0001 || t>=closest.t) { return closest; }
    return Hit(t,u,v,index,closest.instance);
}
fn trace(o:vec3<f32>, d:vec3<f32>, limit:f32, secondary:bool, any_hit:bool) -> Hit {
    var closest=Hit(limit,0.0,0.0,0u,0xffffffffu);
    // Each stack stores only deferred far siblings, with their already tested
    // entry distances. Median CPU trees have <=32 levels, hence <=31 siblings.
    // Recheck the saved distance against a closer hit without repeating slabs.
    var stack:array<vec2<u32>,32>; var depth=0u;
    var current=0u;
    var entry=box_hit(o,d,tlas[0].lo,tlas[0].hi,closest.t);
    loop {
        if(entry<closest.t) {
            let node=tlas[current];
            if(node.count==0u) {
                let left=node.first; let right=left+1u;
                let left_entry=box_hit(o,d,tlas[left].lo,tlas[left].hi,closest.t);
                let right_entry=box_hit(o,d,tlas[right].lo,tlas[right].hi,closest.t);
                let left_nearer=left_entry<=right_entry;
                let near_index=select(right,left,left_nearer);
                let far_index=select(left,right,left_nearer);
                let near_entry=min(left_entry,right_entry);
                let far_entry=max(left_entry,right_entry);
                if(far_entry<closest.t) {
                    stack[depth]=vec2(far_index,bitcast<u32>(far_entry)); depth++;
                }
                if(near_entry<closest.t) { current=near_index; entry=near_entry; continue; }
            } else {
                for(var item=node.first;item<node.first+node.count;item++) {
                    let instance=instances[item];
                    if(secondary && (instance.material.w>0.5 || (any_hit && instance.material.z>0.5))) { continue; }
                    let local_o=(instance.inverse*vec4(o,1.0)).xyz;
                    let local_d=(instance.inverse*vec4(d,0.0)).xyz;
                    var local_stack:array<vec2<u32>,32>; var local_depth=0u;
                    var local_current=instance.root;
                    var local_entry=box_hit(local_o,local_d,nodes[local_current].lo,nodes[local_current].hi,closest.t);
                    loop {
                        if(local_entry<closest.t) {
                            let n=nodes[local_current];
                            if(n.count==0u) {
                                let left=n.first; let right=left+1u;
                                let left_entry=box_hit(local_o,local_d,nodes[left].lo,nodes[left].hi,closest.t);
                                let right_entry=box_hit(local_o,local_d,nodes[right].lo,nodes[right].hi,closest.t);
                                let left_nearer=left_entry<=right_entry;
                                let near_index=select(right,left,left_nearer);
                                let far_index=select(left,right,left_nearer);
                                let near_entry=min(left_entry,right_entry);
                                let far_entry=max(left_entry,right_entry);
                                if(far_entry<closest.t) {
                                    local_stack[local_depth]=vec2(far_index,bitcast<u32>(far_entry)); local_depth++;
                                }
                                if(near_entry<closest.t) { local_current=near_index; local_entry=near_entry; continue; }
                            } else {
                                for(var tri=n.first;tri<n.first+n.count;tri++) {
                                    let hit=triangle_hit(local_o,local_d,tri,closest);
                                    if(hit.t<closest.t) { closest=hit; closest.instance=item; if(any_hit) { return closest; } }
                                }
                            }
                        }
                        if(local_depth==0u) { break; }
                        local_depth--; let next=local_stack[local_depth];
                        local_current=next.x; local_entry=bitcast<f32>(next.y);
                    }
                }
            }
        }
        if(depth==0u) { break; }
        depth--; let next=stack[depth]; current=next.x; entry=bitcast<f32>(next.y);
    }
    return closest;
}
fn environment(d:vec3<f32>) -> vec3<f32> {
    return mix(vec3(0.13,0.18,0.18),vec3(0.66,0.76,0.79),clamp(d.y*0.5+0.5,0.0,1.0));
}
fn normal_at(hit:Hit, d:vec3<f32>) -> vec3<f32> {
    let tri=triangles[hit.triangle];
    let n=tri.n0.xyz*(1.0-hit.u-hit.v)+tri.n1.xyz*hit.u+tri.n2.xyz*hit.v;
    let transformed=normalize((transpose(instances[hit.instance].inverse)*vec4(n,0.0)).xyz);
    let geometric=(transpose(instances[hit.instance].inverse)*vec4(cross(tri.b.xyz-tri.a.xyz,tri.c.xyz-tri.a.xyz),0.0)).xyz;
    return select(transformed,-transformed,dot(geometric,d)>0.0);
}
fn albedo_at(hit:Hit) -> vec3<f32> {
    let tri=triangles[hit.triangle];
    return max(vec3(0.0),(tri.c0.xyz*(1.0-hit.u-hit.v)+tri.c1.xyz*hit.u+tri.c2.xyz*hit.v)*instances[hit.instance].tint.xyz);
}
struct Lighting { diffuse:vec3<f32>, reflection:vec3<f32> }
fn lighting(o:vec3<f32>,d:vec3<f32>,hit:Hit) -> Lighting {
    if(hit.instance==0xffffffffu) { return Lighting(vec3(0.0),vec3(0.038,0.067,0.072)); }
    let material=instances[hit.instance].material; let base=albedo_at(hit);
    let n=normal_at(hit,d); let p=o+d*hit.t;
    // Inlaid UI lettering and enamel share the ray path, with controlled studio illumination.
    if(material.w>0.5) {
        let light=normalize(vec3(-0.35,0.65,0.85));
        return Lighting(vec3(0.85+0.2*max(dot(n,light),0.0)),vec3(0.0));
    }
    if(material.z>0.5) { return Lighting(vec3(1.0),vec3(0.0)); }
    let key=normalize(vec3(-0.45,0.85,0.65));
    let triangle=triangles[hit.triangle];
    var geometric=normalize((transpose(instances[hit.instance].inverse)*vec4(cross(triangle.b.xyz-triangle.a.xyz,triangle.c.xyz-triangle.a.xyz),0.0)).xyz);
    if(dot(geometric,d)>0.0) { geometric=-geometric; }
    let origin=p+geometric*0.008+n*0.06;
    var visible=0u; var shadow_samples=0u;
    // Broad rough ground reveals penumbra bands. Keep articulated/glossy surfaces
    // on six rays, and spend extra visibility samples on that quiet receiving bed.
    let refine_shadow=material.x>0.9 && n.y>0.95;
    let light_u=normalize(cross(vec3(0.0,1.0,0.0),key));
    let light_v=cross(key,light_u);
    for(var s=0u;s<12u;s++) {
        // Interleave the disk: six directions first, then refine its penumbra.
        // Uniformly lit/occluded surfaces retain the original six-ray budget.
        if(s==6u && (!refine_shadow || visible==0u || visible==6u)) { break; }
        let disk_index=(s%6u)*2u+s/6u;
        let angle=f32(disk_index)*2.399963;
        let radius=sqrt((f32(disk_index)+0.5)/12.0)*0.20;
        let light=normalize(key+(light_u*cos(angle)+light_v*sin(angle))*radius);
        let shadow=trace(origin,light,35.0,true,true);
        visible+=select(0u,1u,shadow.instance==0xffffffffu);
        shadow_samples++;
    }
    var visibility=f32(visible)/f32(shadow_samples);
    if(params.size.w==1u) { visibility=1.0; }
    let softness=instances[hit.instance].transmission;
    visibility=mix(visibility,1.0,softness);
    let diffuse=max(dot(n,key),0.0)*visibility;
    var indirect=environment(n)*0.46;
    // One deterministic secondary ray captures nearby color bleeding without history ghosting.
    let tangent=normalize(cross(select(vec3(0.0,1.0,0.0),vec3(1.0,0.0,0.0),abs(n.y)>0.9),n));
    let bounce_dir=normalize(n+tangent*0.45);
    let bounce=trace(origin,bounce_dir,2.5,true,false);
    if(bounce.instance!=0xffffffffu) {
        let bounce_base=albedo_at(bounce); let bn=normal_at(bounce,bounce_dir);
        let proximity=1.0-clamp(bounce.t/2.5,0.0,1.0);
        indirect=mix(indirect, bounce_base*(0.12+max(dot(bn,key),0.0)*0.25),proximity*0.6*(1.0-softness));
    }
    let rough=clamp(material.x,0.1,1.0); let metal=material.y;
    let half_vector=normalize(key-d);
    let spec=pow(max(dot(n,half_vector),0.0),mix(120.0,4.0,rough*rough));
    let fresnel=mix(vec3(0.04),base,metal);
    let reflection_direction=reflect(d,n);
    var reflected=environment(reflection_direction);
    if(metal>0.3 || rough<0.3) {
        let reflected_hit=trace(origin,reflection_direction,20.0,true,false);
        if(reflected_hit.instance!=0xffffffffu) {
            let reflected_normal=normal_at(reflected_hit,reflection_direction);
            let nearby=albedo_at(reflected_hit)*(0.28+0.55*max(dot(reflected_normal,key),0.0));
            reflected=mix(nearby,reflected,rough*rough);
        }
    }
    return Lighting((indirect+vec3(0.95,0.82,0.65)*diffuse*0.85)*(1.0-metal*0.65),
        fresnel*(spec*visibility*(1.8-rough)+reflected*(0.2+0.5*metal)));
}
@compute @workgroup_size(8,8)
fn trace_frame(@builtin(global_invocation_id) gid:vec3<u32>) {
    if(any(gid.xy>=params.size.xy)) { return; }
    var color=vec3(0.0);
    var first=Hit(0.0,0.0,0.0,0u,0xffffffffu);
    var first_normal=vec3(0.0);
    var first_light=Lighting(vec3(0.0),vec3(0.0));
    var samples=4u;
    for(var sample=0u;sample<samples;sample++) {
        let offsets=array<vec2<f32>,4>(vec2(0.25,0.25),vec2(0.75,0.75),vec2(0.75,0.25),vec2(0.25,0.75));
        let offset=offsets[sample];
        let uv=(vec2<f32>(gid.xy)+offset)/vec2<f32>(params.size.xy);
        let clip=vec2(uv.x*2.0-1.0,1.0-uv.y*2.0);
        let near=params.world_from_clip*vec4(clip,1.0,1.0);
        let far=params.world_from_clip*vec4(clip,0.0,1.0);
        let origin=near.xyz/near.w; let direction=normalize(far.xyz/far.w-origin);
        let hit=trace(origin,direction,1500.0,false,false);
        var light=first_light;
        var base=vec3(0.0); var n=vec3(0.0);
        if(hit.instance!=0xffffffffu) { base=albedo_at(hit); n=normal_at(hit,direction); }
        // Visibility is sampled four times. Locally coplanar samples share illumination,
        // never coverage or albedo; letters and silhouette edges retain exact ray coverage.
        if(sample==0u || hit.instance!=first.instance || dot(n,first_normal)<0.995) {
            light=lighting(origin,direction,hit);
        }
        if(sample==0u) {
            first=hit; first_light=light; first_normal=n;
            // Fine inlaid glyphs need four coverage samples; the chunky aquarium uses two.
            if(hit.instance==0xffffffffu) { samples=2u; }
            else if(instances[hit.instance].material.w<0.5) { samples=2u; }
        }
        color+=(base*light.diffuse+light.reflection)/f32(samples);
    }
    textureStore(output,vec2<i32>(gid.xy),vec4(clamp(color,vec3(0.0),vec3(1.0)),1.0));
}
