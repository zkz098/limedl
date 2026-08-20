# Pet Assets — 占位

> 美术资源占位，后续替换为真实 Sprite Sheet。

当前骨架使用 Emoji + CSS 动画占位，无需额外图片。

计划结构：

```
src/pet/assets/default/
  ├─ idle.png        # 4x4, 160x160 per frame, 16 frames
  ├─ work.png
  ├─ celebrate.png
  ├─ sad.png
  ├─ sleep.png
  └─ manifest.json   # { frameWidth: 160, frameHeight: 160, columns: 4, count: 16, fps: 12 }
```

替换时只需：

1. 将文件放入 `src/pet/assets/<model>/`
2. 在 `PetSprite.vue` 中把 Emoji 换为 `background-image` + `background-position: -frame*width`
3. `PetSettings.model` 对应文件夹名
