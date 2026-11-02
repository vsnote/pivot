import os
import math
from PIL import Image, ImageChops, ImageFilter, ImageDraw

def build_official_brand_kit():
    brain_master = '/Users/votan/.gemini/antigravity/brain/c008bfe5-45d4-4b74-a954-a4294de3a3fb/.user_uploaded/media_1793632860255_8dfb4e63.jpg'
    master_path = brain_master if os.path.exists(brain_master) else 'public/app-icon-dark.png'
    master_raw = Image.open(master_path).convert('RGBA')
    W, H = 1024, 1024
    cx, cy = 512, 512

    # 1. Official Apple Squircle (2x oversampled for subpixel precision)
    W2, H2 = 2048, 2048
    cx2, cy2 = 1024, 1024
    r2 = 824.0
    n_squircle = 4.8
    squircle_raw_2x = Image.new('L', (W2, H2), 0)
    for y in range(H2):
        for x in range(W2):
            dx = abs(x - cx2) / r2
            dy = abs(y - cy2) / r2
            if (dx**n_squircle + dy**n_squircle) <= 1.0:
                squircle_raw_2x.putpixel((x, y), 255)
    squircle_mask = squircle_raw_2x.resize((W, H), Image.Resampling.LANCZOS)

    # Physical Shadows
    key_shadow = squircle_mask.filter(ImageFilter.GaussianBlur(24))
    key_layer = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    key_layer.paste(Image.new('RGBA', (W, H), (0, 0, 0, 255)), (0, 16), key_shadow.point(lambda p: int(p * 0.22)))

    amb_shadow = squircle_mask.filter(ImageFilter.GaussianBlur(12))
    amb_layer = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    amb_layer.paste(Image.new('RGBA', (W, H), (0, 0, 0, 255)), (0, 4), amb_shadow.point(lambda p: int(p * 0.12)))

    combined_shadow = Image.alpha_composite(amb_layer, key_layer)
    clean_shadow_alpha = ImageChops.subtract(combined_shadow.split()[3], squircle_mask.point(lambda p: 255 if p > 200 else 0))
    pure_shadow_layer = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    pure_shadow_layer.putalpha(clean_shadow_alpha)

    # 1px rim border mask
    eroded = squircle_mask.point(lambda p: 255 if p > 200 else 0).filter(ImageFilter.MinFilter(3))
    rim_mask = ImageChops.subtract(squircle_mask.point(lambda p: 255 if p > 200 else 0), eroded)

    # 2. Extract Clean Motif Mask from Master
    master_l = master_raw.convert('L')
    motif_alpha = Image.new('L', (W, H), 0)
    for y in range(230, 800):
        for x in range(230, 800):
            val = master_l.getpixel((x, y))
            if val <= 55:
                a = 0
            elif val >= 140:
                a = 255
            else:
                t = (val - 55) / (140 - 55)
                t = t * t * (3 - 2 * t)
                a = int(t * 255)
            motif_alpha.putpixel((x, y), a)

    print('Clean motif alpha extracted:', motif_alpha.getbbox())

    # 3. Build Dark Mode Icon
    # The master image already has the exact dark plate and white motif!
    # Mask master image strictly with squircle_mask
    dark_entity = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    dark_entity.paste(master_raw, (0, 0), squircle_mask)
    # Add subtle 1px ambient highlight rim
    dark_entity.paste(Image.new('RGBA', (W, H), (255, 255, 255, 28)), (0, 0), rim_mask)
    # Composite with pure shadow
    dark_app_icon = Image.alpha_composite(pure_shadow_layer, dark_entity)
    dark_app_icon.save('public/app-icon-dark.png', 'PNG')
    print('Generated public/app-icon-dark.png')

    dark_app_icon.save('public/app-icon.png', 'PNG')
    print('Updated public/app-icon.png')

    # 4. Build Light Mode Icon (White Squircle, Tactile Black Motif)
    # Background plate: white with subtle daylight vertical gradient
    light_plate = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    for y in range(H):
        factor = max(0.0, min(1.0, (y - 100) / 824.0))
        val = int(255 - factor * 8)
        for x in range(100, 925):
            light_plate.putpixel((x, y), (val, val, val + 1, 255))
    light_entity = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    light_entity.paste(light_plate, (0, 0), squircle_mask)
    # Subtle dark 1px boundary
    light_entity.paste(Image.new('RGBA', (W, H), (0, 0, 0, 20)), (0, 0), rim_mask)

    # Invert motif lighting for light mode:
    # In master, bright pixels are white motif, shading is darker.
    # For light mode, we create a rich tactile obsidian black motif:
    # Background of plate is white.
    # Where motif_alpha > 0, we render dark tones (rgb: 12-28) with subtle shading derived from master
    light_motif = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    for y in range(230, 800):
        for x in range(230, 800):
            a = motif_alpha.getpixel((x, y))
            if a > 0:
                # Master luminance
                l_val = master_l.getpixel((x, y))
                # Map highlight in master to slight specular highlight on black plate
                # Normal dark tone: 16. Specular edge: up to 38.
                factor = max(0.0, min(1.0, (l_val - 120) / 125.0))
                dark_val = int(14 + factor * 18)
                light_motif.putpixel((x, y), (dark_val, dark_val, dark_val + 2, a))

    # Add soft micro-contact shadow under black motif on white plate
    motif_shadow = motif_alpha.filter(ImageFilter.GaussianBlur(10))
    shadow_under_motif = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    shadow_under_motif.paste(Image.new('RGBA', (W, H), (0, 0, 0, 45)), (0, 6), motif_shadow)

    light_entity = Image.alpha_composite(light_entity, shadow_under_motif)
    light_entity = Image.alpha_composite(light_entity, light_motif)
    light_app_icon = Image.alpha_composite(pure_shadow_layer, light_entity)
    light_app_icon.save('public/app-icon-light.png', 'PNG')
    print('Generated public/app-icon-light.png')

    # 5. Build Transparent In-App Brandmarks (512x512)
    # Dark Mode Brandmark: White motif with transparency
    # Extract white motif from master
    white_mark_full = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    for y in range(230, 800):
        for x in range(230, 800):
            a = motif_alpha.getpixel((x, y))
            if a > 0:
                p = master_raw.getpixel((x, y))
                white_mark_full.putpixel((x, y), (p[0], p[1], p[2], a))

    # Resize to 512x512
    brand_mark_dark = white_mark_full.resize((512, 512), Image.Resampling.LANCZOS)
    brand_mark_dark.save('public/brand-mark-dark.png', 'PNG')
    print('Generated public/brand-mark-dark.png')

    # Light Mode Brandmark: Black motif with transparency
    black_mark_full = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    black_mark_full.paste(light_motif, (0, 0))
    brand_mark_light = black_mark_full.resize((512, 512), Image.Resampling.LANCZOS)
    brand_mark_light.save('public/brand-mark-light.png', 'PNG')
    print('Generated public/brand-mark-light.png')

    # 6. Status Tray Icons for macOS Menu Bar (44x44 and 22x22)
    # Clean Template: pure black with alpha
    bbox = motif_alpha.getbbox()
    motif_cropped = motif_alpha.crop(bbox)
    
    # 44x44 (@2x): 34px motif centered
    tray_size_44 = 34
    alpha_44 = motif_cropped.resize((tray_size_44, tray_size_44), Image.Resampling.LANCZOS)
    tray_44 = Image.new('RGBA', (44, 44), (0, 0, 0, 0))
    off_44 = (44 - tray_size_44) // 2
    tray_44.paste(Image.new('RGBA', (tray_size_44, tray_size_44), (0, 0, 0, 255)), (off_44, off_44), alpha_44)
    tray_44.save('src-tauri/icons/tray-icon.png', 'PNG')
    print('Generated src-tauri/icons/tray-icon.png (44x44 @2x Template)')

    # 22x22 (@1x): 18px motif centered
    tray_size_22 = 18
    alpha_22 = motif_cropped.resize((tray_size_22, tray_size_22), Image.Resampling.LANCZOS)
    tray_22 = Image.new('RGBA', (22, 22), (0, 0, 0, 0))
    off_22 = (22 - tray_size_22) // 2
    tray_22.paste(Image.new('RGBA', (tray_size_22, tray_size_22), (0, 0, 0, 255)), (off_22, off_22), alpha_22)
    tray_22.save('src-tauri/icons/tray-icon-22.png', 'PNG')
    print('Generated src-tauri/icons/tray-icon-22.png (22x22 @1x Template)')

    brain_dir = '/Users/votan/.gemini/antigravity/brain/c008bfe5-45d4-4b74-a954-a4294de3a3fb'
    if os.path.isdir(brain_dir):
        dark_app_icon.save(os.path.join(brain_dir, 'official-pivot-app-icon-dark.png'), 'PNG')
        light_app_icon.save(os.path.join(brain_dir, 'official-pivot-app-icon-light.png'), 'PNG')

if __name__ == '__main__':
    build_official_brand_kit()
