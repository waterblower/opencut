#include <metal_stdlib>
using namespace metal;

// GPUI's existing surface shader expects 8-bit full-range BT.601 NV12.
// Each invocation writes a 2x2 luma block and its averaged chroma pair.
kernel void prepare_surface(
    texture2d<float, access::read> input_y [[texture(0)]],
    texture2d<float, access::read> input_uv [[texture(1)]],
    texture2d<float, access::write> output_y [[texture(2)]],
    texture2d<float, access::write> output_uv [[texture(3)]],
    constant float4 *transform [[buffer(0)]],
    uint2 block [[thread_position_in_grid]]) {
    if (block.x >= output_uv.get_width() || block.y >= output_uv.get_height()) return;
    float2 uv = input_uv.read(block).rg;
    float2 chroma = 0.0;
    // Inverse of GPUI's exact, rounded BT.601 coefficients, including its 0.5
    // chroma offset. Do not rely on GPUI reading pixel-buffer color attachments.
    float3 weights = float3(0.7141 / 1.402, 1.0, 0.3441 / 1.772);
    weights /= weights.x + weights.y + weights.z;
    for (uint y = 0; y < 2; ++y) {
        for (uint x = 0; x < 2; ++x) {
            uint2 position = block * 2 + uint2(x, y);
            float4 sample = float4(input_y.read(position).r, uv, 1.0);
            float3 rgb = float3(dot(transform[0], sample),
                                dot(transform[1], sample),
                                dot(transform[2], sample));
            float luma = dot(weights, rgb);
            output_y.write(float4(luma, 0.0, 0.0, 1.0), position);
            chroma += float2((rgb.b - luma) / 1.772, (rgb.r - luma) / 1.402) + 0.5;
        }
    }
    output_uv.write(float4(chroma * 0.25, 0.0, 1.0), block);
}
