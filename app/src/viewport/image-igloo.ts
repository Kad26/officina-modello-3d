import * as THREE from "three";
import type { SavedView } from "../engine";

export type MatchedView = SavedView & { photo: string };

export function matchedViews(views: SavedView[]): MatchedView[] {
  return views.filter((view): view is MatchedView => !!view.photo
    && [...view.eye, ...view.target].every(Number.isFinite)
    && view.eye.some((v, i) => v !== view.target[i]));
}

type PhotoCard = {
  view: MatchedView;
  eye: THREE.Vector3;
  direction: THREE.Vector3;
  mesh: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>;
  border: THREE.LineSegments<THREE.EdgesGeometry, THREE.LineBasicMaterial>;
};

const Z_UP = new THREE.Vector3(0, 0, 1);
const frame = new THREE.Color(0x28bfbb);
const muted = new THREE.Color(0x738a98);

export class ImageIgloo {
  readonly group = new THREE.Group();
  private readonly cards: PhotoCard[];
  private focused: PhotoCard;
  private disposed = false;

  constructor(views: MatchedView[], active: MatchedView, readonly pivot: THREE.Vector3) {
    this.group.name = "Matched photo overview";
    const loader = new THREE.TextureLoader();
    this.cards = views.map((view) => {
      const eye = new THREE.Vector3(...view.eye);
      const target = new THREE.Vector3(...view.target);
      const beam = target.sub(eye);
      const distance = beam.length();
      const direction = eye.clone().sub(pivot).normalize();
      if (!direction.lengthSq()) direction.copy(beam).negate().normalize();
      const position = eye.clone().addScaledVector(beam, 0.18);
      const height = THREE.MathUtils.clamp(distance * 0.19, 0.25, 500);
      const photo = loader.load(view.photo, (texture) => {
        if (this.disposed) return;
        const aspect = texture.image.width / Math.max(1, texture.image.height);
        mesh.scale.x = THREE.MathUtils.clamp(aspect, 0.2, 5);
      });
      photo.colorSpace = THREE.SRGBColorSpace;
      const geometry = new THREE.PlaneGeometry(height, height);
      const material = new THREE.MeshBasicMaterial({ map: photo, side: THREE.DoubleSide, transparent: true, depthWrite: false, toneMapped: false });
      const mesh = new THREE.Mesh(geometry, material);
      mesh.position.copy(position);
      mesh.up.copy(view.up ? new THREE.Vector3(...view.up).normalize() : Z_UP);
      mesh.lookAt(eye);
      const border = new THREE.LineSegments(new THREE.EdgesGeometry(geometry), new THREE.LineBasicMaterial({ transparent: true, depthWrite: false, toneMapped: false }));
      mesh.add(border);
      this.group.add(mesh);
      return { view, eye, direction, mesh, border };
    });
    this.focused = this.cards.find((card) => card.view === active) ?? this.cards[0];
    this.highlight(this.focused);
  }

  get selected(): MatchedView {
    return this.focused.view;
  }

  get direction(): THREE.Vector3 {
    return this.focused.direction;
  }

  private highlight(next: PhotoCard) {
    this.focused = next;
    for (const card of this.cards) {
      const active = card === next;
      card.mesh.material.opacity = active ? 0.83 : 0.34;
      card.mesh.renderOrder = active ? 3 : 1;
      card.border.renderOrder = active ? 4 : 2;
      card.border.material.color.copy(active ? frame : muted);
      card.border.material.opacity = active ? 1 : 0.56;
    }
  }

  highlightFrom(eye: THREE.Vector3) {
    const direction = eye.clone().sub(this.pivot).normalize();
    let best = this.focused;
    let score = direction.dot(best.direction);
    for (const card of this.cards) {
      const next = direction.dot(card.direction);
      // Keep the highlighted photo when views share a camera position.
      if (next > score + 1e-5) {
        score = next;
        best = card;
      }
    }
    if (best === this.focused) return false;
    this.highlight(best);
    return true;
  }

  step(key: string, order: "spatial" | "scene" | "vantage") {
    const azimuth = ({ direction: d }: PhotoCard) => Math.atan2(d.y, d.x);
    const elevation = ({ direction: d }: PhotoCard) => d.z;
    const field = key === "ArrowUp" || key === "ArrowDown" ? elevation : azimuth;
    const ordered = order === "spatial"
      ? [...this.cards].sort((a, b) => field(a) - field(b))
      : order === "vantage"
        ? this.cards.filter((card) => card.eye.distanceTo(this.focused.eye) < 0.01)
        : this.cards;
    if (ordered.length < 2) return false;
    const index = ordered.indexOf(this.focused);
    const step = key === "ArrowLeft" || key === "ArrowDown" ? -1 : 1;
    this.highlight(ordered[(index + step + ordered.length) % ordered.length]);
    return true;
  }

  pick(ray: THREE.Raycaster) {
    const hit = ray.intersectObjects(this.cards.map((card) => card.mesh), false)[0];
    const card = hit && this.cards.find((candidate) => candidate.mesh === hit.object);
    if (!card) return false;
    this.highlight(card);
    return true;
  }

  bounds() {
    const bounds = new THREE.Box3().setFromObject(this.group);
    return bounds.expandByPoint(this.pivot);
  }

  dispose() {
    this.disposed = true;
    this.group.removeFromParent();
    for (const { mesh, border } of this.cards) {
      mesh.geometry.dispose();
      mesh.material.map?.dispose();
      mesh.material.dispose();
      border.geometry.dispose();
      border.material.dispose();
    }
  }
}
