// Render posed statue models (render_portraits.py writes one JSON per statue) to transparent PNGs
// with three.js in headless Chromium (Playwright). Unlit textures, as the game's toon shader
// bakes its shading into them, plus an inverted-hull outline.
//
// usage: node tools/content/render_portraits.js <three.min.js> <jobs.json>
//   jobs.json: [{"in": "<statue>.json", "out": "<face>.png", "opt": {...}}]
//   opt: fov (deg), dist (m), dx/dy (m, aim offset from the HEAD bone), size (px),
//        onlyModel (0 = the character, not the Stand), outline (m)
// Unity is left-handed (z forward), three.js right-handed: z is negated and triangles reversed.
const fs = require('fs');
const { chromium } = require('playwright');

(async () => {
  const [three, jobsPath] = process.argv.slice(2);
  const jobs = JSON.parse(fs.readFileSync(jobsPath, 'utf8'));
  const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const size = (jobs[0] && jobs[0].opt.size) || 512;
  const page = await browser.newPage({ viewport: { width: size, height: size } });
  page.on('console', m => { if (!/deprecated/.test(m.text())) console.error('page:', m.text()); });
  await page.setContent(`<html><body style="margin:0;background:transparent"><canvas id=c width=${size} height=${size}></canvas></body></html>`);
  await page.addScriptTag({ content: fs.readFileSync(three, 'utf8') });
  for (const job of jobs) {
    const data = JSON.parse(fs.readFileSync(job.in, 'utf8'));
    const png = await page.evaluate(async ({ data, opt }) => {
      const b64 = s => Uint8Array.from(atob(s), c => c.charCodeAt(0)).buffer;
      window.R = window.R || new THREE.WebGLRenderer({ canvas: document.getElementById('c'), antialias: true, alpha: true, preserveDrawingBuffer: true });
      const R = window.R;
      R.setClearColor(0x000000, 0);
      R.outputColorSpace = THREE.SRGBColorSpace;
      const scene = new THREE.Scene();
      const loadTex = src => new Promise(res => {
        const im = new Image();
        im.onload = () => { const t = new THREE.Texture(im); t.colorSpace = THREE.SRGBColorSpace; t.flipY = true; t.needsUpdate = true; res(t); };
        im.src = 'data:image/png;base64,' + src;
      });
      for (const p of data.parts) {
        if (opt.onlyModel !== undefined && p.model !== opt.onlyModel) continue;
        const pos = new Float32Array(b64(p.pos));
        for (let i = 2; i < pos.length; i += 3) pos[i] = -pos[i];
        const idx = new Uint32Array(b64(p.idx));
        for (let i = 0; i < idx.length; i += 3) { const t = idx[i + 1]; idx[i + 1] = idx[i + 2]; idx[i + 2] = t; }
        const g = new THREE.BufferGeometry();
        g.setAttribute('position', new THREE.BufferAttribute(pos, 3));
        g.setAttribute('uv', new THREE.BufferAttribute(new Float32Array(b64(p.uv)), 2));
        g.setIndex(new THREE.BufferAttribute(idx, 1));
        g.computeVertexNormals();
        const tex = p.png ? await loadTex(p.png) : null;
        scene.add(new THREE.Mesh(g, new THREE.MeshBasicMaterial({ map: tex, color: tex ? 0xffffff : 0x888888, alphaTest: 0.5 })));
        if (opt.outline) {
          const og = g.clone(), n = og.attributes.normal, q = og.attributes.position;
          for (let i = 0; i < q.count; i++) {
            q.setXYZ(i, q.getX(i) + n.getX(i) * opt.outline, q.getY(i) + n.getY(i) * opt.outline, q.getZ(i) + n.getZ(i) * opt.outline);
          }
          scene.add(new THREE.Mesh(og, new THREE.MeshBasicMaterial({ color: 0x101010, side: THREE.BackSide })));
        }
      }
      const H = data.head;
      const target = new THREE.Vector3(H[0][3] + (opt.dx || 0), H[1][3] + (opt.dy || 0), -H[2][3]);
      const cam = new THREE.PerspectiveCamera(opt.fov || 20, 1, 0.01, 100);
      // the model faces Unity +z (three.js -z); the camera stands in front of it
      cam.position.copy(target).add(new THREE.Vector3(0, 0, -(opt.dist || 0.95)));
      cam.lookAt(target);
      R.render(scene, cam);
      return document.getElementById('c').toDataURL('image/png');
    }, { data, opt: job.opt });
    fs.writeFileSync(job.out, Buffer.from(png.split(',')[1], 'base64'));
  }
  await browser.close();
})();
