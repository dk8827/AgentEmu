struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var xy = array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));
    var out: VertexOutput;
    out.position=vec4<f32>(xy[index],0.0,1.0);
    out.uv=vec2<f32>((xy[index].x+1.0)*0.5,(1.0-xy[index].y)*0.5);
    return out;
}
@group(0) @binding(0) var framebuffer: texture_2d<f32>;
@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel=clamp(vec2<i32>(in.uv*vec2<f32>(256.0,240.0)),vec2<i32>(0),vec2<i32>(255,239));
    return textureLoad(framebuffer,pixel,0);
}
