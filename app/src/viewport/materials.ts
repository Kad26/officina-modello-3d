import * as THREE from "three";

/** Back faces pushed out by a few screen pixels: a silhouette outline. */
export function profileMaterial(color: number, px: number, clip: THREE.Plane[]): THREE.MeshBasicMaterial {
  const m = new THREE.MeshBasicMaterial({ color, side: THREE.BackSide, clippingPlanes: clip });
  m.onBeforeCompile = (shader) => {
    shader.uniforms.profilePx = { value: px };
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nuniform float profilePx;")
      .replace(
        "#include <project_vertex>",
        `#include <project_vertex>
        vec4 clipN = projectionMatrix * modelViewMatrix * vec4(normal, 0.0);
        vec2 dir = length(clipN.xy) > 1e-6 ? normalize(clipN.xy) : vec2(0.0);
        // pixels -> clip space (approximate viewport of 1000 px tall)
        gl_Position.xy += dir * profilePx * gl_Position.w / 500.0;`
      );
  };
  return m;
}

// images for textured faces, loaded once per material and image
const textureCache = new Map<string, THREE.Texture>();
export function textureFor(name: string, data: string): THREE.Texture {
  const key = `${name}:${data.length}:${data.slice(-32)}`;
  let t = textureCache.get(key);
  if (!t) {
    t = new THREE.TextureLoader().load(data);
    t.wrapS = t.wrapT = THREE.RepeatWrapping;
    t.colorSpace = THREE.SRGBColorSpace;
    t.anisotropy = 8;
    textureCache.set(key, t);
  }
  return t;
}
