#version 150 core

in vec4 v_Color;
in vec3 v_Pos;
out vec4 Target0;

layout (std140) uniform Locals {
    mat4 u_Transform;
    float u_HighestDim;
    float u_Lacunarity;
    float u_Octaves;
    float u_Offset;
    float u_Gain;
};

// First few functions lifted from
// <https://gist.github.com/patriciogonzalezvivo/670c22f3966e662d2f83>

float mod289(float f) {
    return f - floor(f * (1.0 / 289.0)) * 289.0;
}

vec4 mod289(vec4 p) {
    return p - floor(p * (1.0 / 289.0)) * 289.0;
}

vec4 perm(vec4 p) {
    return mod289(((p * 34.0) + 1.0) * p);
}

float noise(vec3 p) {
    vec3 a = floor(p);
    vec3 d = p - a;
    d = d * d * (3.0 - 2.0 * d);

    vec4 b = a.xxyy + vec4(0.0, 1.0, 0.0, 1.0);
    vec4 k1 = perm(b.xyxy);
    vec4 k2 = perm(k1.xyxy + b.zzww);

    vec4 c = k2 + a.zzzz;
    vec4 k3 = perm(c);
    vec4 k4 = perm(c + 1.0);

    vec4 o1 = fract(k3 * (1.0 / 41.0));
    vec4 o2 = fract(k4 * (1.0 / 41.0));

    vec4 o3 = o2 * d.z + o1 * (1.0 - d.z);
    vec2 o4 = o3.yw * d.x + o3.xz * (1.0 - d.x);

    return o4.y * d.y + o4.x * (1.0 - d.y);
}

// Lifted from Blender source
// https://git.blender.org/gitweb/gitweb.cgi/blender.git/blob/HEAD:/source/blender/gpu/shaders/gpu_shader_material.glsl#l2624

float fractal(vec3 point) {
    float result = noise(point) + u_Offset;
    float attenuation = pow(u_Lacunarity, -u_HighestDim);
    float power = attenuation;

    float weight = u_Gain * result;
    point *= u_Lacunarity;

    int octaves = int(u_Octaves);

    for (int i = 0; i < octaves; i++) {
        if (weight > 1.0) {
            weight = 1.0;
        }

        float signal = (noise(point) + u_Offset) * power;
        power *= attenuation;
        result += weight * signal;
        weight *= u_Gain * signal;
        point *= u_Lacunarity;
    }

    float remainder = u_Octaves - octaves;
    if (remainder != 0.0) {
        result += remainder * ((noise(point) + u_Offset) * power);
    }

    return result;
}

void main() {
    Target0 = v_Color * fractal(v_Pos);
}
