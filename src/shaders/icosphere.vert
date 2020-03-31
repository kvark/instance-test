#version 450

layout(set = 0, binding = 0) uniform Globals {
    mat4 u_Transform;
};

layout(location = 0) in vec3 a_Pos;
layout(location = 1) in vec3 a_Normal;

layout(location = 0) out vec4 v_Normal;

void main() {
    v_Normal = vec4(a_Normal, 0.0);
    vec4 pos = vec4(a_Pos.x, a_Pos.y, a_Pos.z, 1.0);
    gl_Position = u_Transform * pos;
    // gl_Position = vec4(0.0, 0.0, 0.5, 1.0);
}