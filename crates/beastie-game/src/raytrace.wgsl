// Geometry, letters and interface all intersect in this same ordinary-compute path.
struct Triangle { a:vec4<f32>, b:vec4<f32>, c:vec4<f32>, n0:vec4<f32>, n1:vec4<f32>, n2:vec4<f32>, c0:vec4<f32>, c1:vec4<f32>, c2:vec4<f32> }
struct Node { lo:vec3<f32>, first:u32, hi:vec3<f32>, count:u32 }
struct Instance { world:mat4x4<f32>, inverse:mat4x4<f32>, material:vec4<f32>, tint:vec4<f32>, root:u32, transmission:f32, pad1:u32, pad2:u32 }
struct Params { world_from_clip:mat4x4<f32>, size:vec4<u32>, water:vec4<f32> }
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
// Moving irregular cell boundaries approximate focused surface light without a
// repeated sinusoidal lattice. Stable hash positions keep this deterministic.
fn water_hash(p:vec2<f32>) -> vec2<f32> {
    return fract(sin(vec2(dot(p,vec2(127.1,311.7)),dot(p,vec2(269.5,183.3))))*43758.5453);
}
fn water_noise(p:vec2<f32>) -> f32 {
    let cell=floor(p); let f=fract(p); let u=f*f*(3.0-2.0*f);
    return mix(mix(water_hash(cell).x,water_hash(cell+vec2(1.0,0.0)).x,u.x),
        mix(water_hash(cell+vec2(0.0,1.0)).x,water_hash(cell+vec2(1.0,1.0)).x,u.x),u.y);
}
fn caustic(p:vec2<f32>, time:f32) -> f32 {
    let drift=vec2(time*0.035,-time*0.026);
    let warp=vec2(water_noise(p*2.3+drift),water_noise(p.yx*2.1-drift+vec2(5.2)))-0.5;
    let warped=p*1.16+warp*1.25+vec2(sin(p.y*2.4+time*0.17),sin(p.x*1.8-time*0.13))*0.16;
    let cell=floor(warped); let local=fract(warped);
    var first=8.0; var second=8.0;
    for(var y=-1;y<=1;y++) { for(var x=-1;x<=1;x++) {
        let neighbor=vec2<f32>(f32(x),f32(y));
        let random=water_hash(cell+neighbor);
        let site=neighbor+0.5+sin(random*6.283185+time*0.20)*0.34;
        let delta=site-local; let distance=dot(delta,delta);
        if(distance<first) { second=first; first=distance; }
        else { second=min(second,distance); }
    } }
    let variation=water_noise(p*3.1+drift);
    let edge=1.0-smoothstep(0.012,0.10+variation*0.22,second-first);
    let breakup=0.18+0.82*smoothstep(0.18,0.82,water_noise(p*0.62-drift+vec2(9.7)));
    return edge*edge*breakup;
}
fn water_optics(color:vec3<f32>, o:vec3<f32>, d:vec3<f32>, hit:Hit) -> vec3<f32> {
    if(hit.instance==0xffffffffu || instances[hit.instance].material.w>0.5) { return color; }
    let p=o+d*hit.t;
    if(abs(p.x)>7.75 || p.y < -2.13 || p.y>4.60 || p.z>2.15) { return color; }
    let time=params.water.x;
    let depth=clamp(2.1-p.z,0.0,4.8);
    let transmission=exp(-vec3(0.075,0.033,0.018)*depth);
    let high=clamp((p.y+2.0)/6.6,0.0,1.0);
    let ambient=mix(vec3(0.009,0.040,0.047),vec3(0.018,0.105,0.111),high);
    var result=color*transmission+ambient*(vec3(1.0)-transmission)*0.65;
    // Broad slanted shafts are strongest behind objects and dissolve before the bed.
    let spread=4.6-p.y;
    var shafts=0.0;
    let veil=0.83+0.17*sin(p.y*1.45+sin(p.x*0.7)+time*0.032);
    for(var i=0u;i<4u;i++) {
        let source=array<f32,4>(-5.9,-2.8,2.0,5.8)[i];
        let center=source+spread*(0.11+f32(i)*0.015);
        let width=0.11+f32(i%2u)*0.13+spread*(0.052+f32(i)*0.011);
        let dist=(p.x-center)/width;
        shafts+=exp(-dist*dist)*exp(-spread*0.23)*(0.85+0.15*sin(time*0.18+f32(i)*2.1))*veil;
    }
    result+=vec3(0.12,0.30,0.29)*shafts*0.74*clamp(depth/3.0,0.0,1.0);
    if(instances[hit.instance].material.z>0.5 && p.z < -2.0) {
        let center=exp(-p.x*p.x*0.028);
        result*=1.0-(1.0-high)*center*0.32;
        result+=vec3(0.005,0.052,0.063)*high*high*center;
    }
    // Analytic thin surface: its distorted normals and broken Fresnel highlights
    // animate without rebuilding a mesh or adding a second traversal.
    if(abs(d.y)>0.0001) {
        let surface_t=(4.02-o.y)/d.y;
        let surface=o+d*surface_t;
        if(surface_t>0.0 && surface_t<hit.t && abs(surface.x)<7.74 && surface.z > -0.6 && surface.z<2.10) {
            let q=surface.xz;
            let drift=vec2(time*0.034,-time*0.024);
            // Independent ripple scales break reflections into small patches. Avoid
            // thresholding one broad warped field, which reads as marbled metal.
            let warp=q+vec2(sin(q.y*6.0+q.x*0.7+time*0.22),
                sin(q.x*3.7-q.y*1.8-time*0.19))*0.065;
            let broad=water_noise(warp*vec2(3.6,6.2)+drift);
            let fine=water_noise(warp*vec2(13.0,19.0)-drift*1.7);
            let ripple=0.5+0.5*sin(q.x*12.4+q.y*26.0+sin(q.x*4.8-q.y*7.1)*1.8+time*0.38);
            let patches=smoothstep(0.58,0.79,broad*0.54+fine*0.31+ripple*0.15);
            let glint=pow(smoothstep(0.65,0.90,fine*0.72+ripple*0.28),2.0);
            var lamp=0.0;
            for(var i=0u;i<4u;i++) {
                let x=array<f32,4>(-6.9,-4.2,4.6,7.0)[i];
                lamp+=exp(-(q.x-x)*(q.x-x)*2.4);
            }
            let center=exp(-q.x*q.x*0.055);
            let warmth=clamp(lamp*0.9,0.0,1.0);
            let sparkle=mix(vec3(0.34,0.72,0.66),vec3(0.95,0.73,0.34),warmth);
            let water=vec3(0.005,0.045,0.052)+vec3(0.008,0.034,0.033)*(broad+center*0.5);
            let surface_color=water+sparkle*(patches*0.95+glint*0.30)*(0.55+lamp*0.40+center*0.28)*params.water.y;
            let edge=1.0-smoothstep(1.65,2.1,surface.z);
            result=mix(result,surface_color,0.80+edge*0.11);

        }
    }
    return result;
}
fn environment(d:vec3<f32>) -> vec3<f32> {
    return mix(vec3(0.10,0.17,0.16),vec3(0.60,0.77,0.78),clamp(d.y*0.5+0.5,0.0,1.0));
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
        let radius=sqrt((f32(disk_index)+0.5)/12.0)*0.25;
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
    var indirect=environment(n)*0.30;
    // One deterministic secondary ray captures nearby color bleeding without history ghosting.
    let tangent=normalize(cross(select(vec3(0.0,1.0,0.0),vec3(1.0,0.0,0.0),abs(n.y)>0.9),n));
    let bounce_dir=normalize(n+tangent*0.45);
    let bounce=trace(origin,bounce_dir,2.5,true,false);
    if(bounce.instance!=0xffffffffu) {
        let bounce_base=albedo_at(bounce); let bn=normal_at(bounce,bounce_dir);
        let proximity=1.0-clamp(bounce.t/2.5,0.0,1.0);
        indirect=mix(indirect, bounce_base*(0.12+max(dot(bn,key),0.0)*0.25),proximity*0.6*(1.0-softness));
    }
    // Reuse the existing short diffuse bounce for contact darkening. A second
    // occlusion traversal adds too much cost on ordinary-compute ray tracing.
    let contact=select(0.0,1.0-smoothstep(0.06,0.85,bounce.t),bounce.instance!=0xffffffffu);
    indirect*=1.0-contact*0.48*(1.0-softness);
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
    // Caustics mostly belong on the bed and lower upward-facing scenery.
    let caustic_weight=max(n.y,0.0)*exp(-max(p.y+1.75,0.0)*0.85)*params.water.y;
    var caustic_light=0.0;
    if(caustic_weight>0.025 && visibility>0.0) {
        caustic_light=caustic(p.xz,params.water.x)*caustic_weight*visibility;
    }
    // Bounded practical fill beneath the four warm top lamps. Keep it softer and
    // weaker than the shadowed aquarium key so the water remains cool.
    var practical=0.0;
    for(var i=0u;i<4u;i++) {
        let x=array<f32,4>(-6.9,-4.2,4.6,7.0)[i];
        let delta=vec3(x,4.45,1.3)-p;
        practical+=max(dot(n,normalize(delta)),0.0)/(1.0+dot(delta,delta)*0.48);
    }
    let warm_edge=pow(clamp(abs(p.x)/7.8,0.0,1.0),8.0)*0.08+practical*0.24;
    var grain=1.0;
    if(p.y < -1.7 && n.y>0.9 && material.x>0.9) {
        grain=0.95+water_hash(floor(p.xz*65.0)).x*0.10;
    }
    // Broad quiet rim assistance catches the fold planes of green leaves.
    if(base.g>base.r*1.2 && base.g>base.b*1.15 && material.w<0.5) {
        indirect+=vec3(0.16,0.23,0.12)*(0.12+0.24*abs(n.x));
    }
    return Lighting((indirect+vec3(1.0,0.89,0.70)*diffuse*1.08
        +vec3(1.0,0.91,0.66)*caustic_light*1.15+vec3(0.8,0.48,0.18)*warm_edge)*(1.0-metal*0.65)*grain,
        fresnel*(spec*visibility*(1.8-rough)+reflected*(0.2+0.5*metal)));
}
@compute @workgroup_size(8,8)
fn trace_frame(@builtin(global_invocation_id) gid:vec3<u32>) {
    if(any(gid.xy>=params.size.xy)) { return; }
    var color=vec3(0.0);
    var first=Hit(0.0,0.0,0.0,0u,0xffffffffu);
    var first_normal=vec3(0.0);
    var first_light=Lighting(vec3(0.0),vec3(0.0));
    var behind=vec3(0.0); var behind_ready=false;
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
        var sample_color=water_optics(base*light.diffuse+light.reflection,origin,direction,hit);
        if(hit.instance!=0xffffffffu && instances[hit.instance].material.w>0.5
            && instances[hit.instance].transmission>0.0) {
            // Only the large settings plate opts in. One world-only continuation
            // per pixel retains aquarium context; inlaid text remains opaque.
            if(!behind_ready) {
                let world_hit=trace(origin,direction,1500.0,true,false);
                if(world_hit.instance!=0xffffffffu) {
                    let world_base=albedo_at(world_hit);
                    let world_normal=normal_at(world_hit,direction);
                    let key=normalize(vec3(-0.45,0.85,0.65));
                    var illumination=environment(world_normal)*0.46
                        +vec3(1.0,0.89,0.70)*max(dot(world_normal,key),0.0)*1.08;
                    if(instances[world_hit.instance].material.z>0.5) { illumination=vec3(1.0); }
                    behind=water_optics(world_base*illumination,origin,direction,world_hit);
                }
                behind_ready=true;
            }
            sample_color=mix(sample_color,behind,clamp(instances[hit.instance].transmission,0.0,0.25));
        }
        color+=sample_color/f32(samples);
    }
    textureStore(output,vec2<i32>(gid.xy),vec4(clamp(color,vec3(0.0),vec3(1.0)),1.0));
}
