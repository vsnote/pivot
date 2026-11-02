import math
from PIL import Image, ImageChops, ImageFilter, ImageDraw

def create_desktop_icons():
    # 1. Load base icon and extract official macOS squircle mask & shadow
    base_icon = Image.open('public/app-icon.png').convert('RGBA')
    alpha_orig = base_icon.split()[3]
    squircle_mask = alpha_orig.point(lambda p: 255 if p == 255 else 0)

    # 2. Extract shadow layer (pure black with original ambient + key shadow alpha)
    shadow_alpha = ImageChops.subtract(alpha_orig, squircle_mask)
    shadow_layer = Image.new('RGBA', (1024, 1024), (0, 0, 0, 0))
    shadow_layer.putalpha(shadow_alpha)

    # 3. Create squircle border highlight / rim mask (1px inner border)
    # Erode squircle mask by 1.5px to get inner boundary
    eroded_mask = squircle_mask.filter(ImageFilter.MinFilter(3))
    rim_mask = ImageChops.subtract(squircle_mask, eroded_mask)

    # 4. Load motif from brand-mark-light.png and scale to optimal size (~500px width)
    bml = Image.open('public/brand-mark-light.png').convert('RGBA')
    motif_alpha = bml.split()[3]
    motif_box = motif_alpha.getbbox() # (46, 91, 466, 421)
    motif_cropped = motif_alpha.crop(motif_box)
    
    # Scale motif
    target_width = 490
    target_height = int(motif_cropped.height * (target_width / motif_cropped.width))
    scaled_motif_alpha = motif_cropped.resize((target_width, target_height), Image.Resampling.LANCZOS)
    
    # Center coordinates in 1024x1024
    pos_x = (1024 - target_width) // 2
    pos_y = (1024 - target_height) // 2

    # --- 5. Generate LIGHT MODE App Icon (White background, Black motif) ---
    # Create white background with subtle vertical gradient
    light_bg = Image.new('RGBA', (1024, 1024), (0, 0, 0, 0))
    for y in range(1024):
        # Subtle gradient from #ffffff at top (100) to #f6f8fa at bottom (924)
        factor = max(0.0, min(1.0, (y - 100) / 824.0))
        val = int(255 - factor * 8) # 255 down to 247
        for x in range(100, 925):
            light_bg.putpixel((x, y), (val, val, val + 1, 255))
    
    # Apply squircle mask to background
    light_entity = Image.new('RGBA', (1024, 1024), (0, 0, 0, 0))
    light_entity.paste(light_bg, (0, 0), squircle_mask)

    # Add 1px subtle dark border around light squircle for crisp edge on white wallpaper
    border_layer_light = Image.new('RGBA', (1024, 1024), (0, 0, 0, 22)) # rgba(0, 0, 0, 0.086)
    light_entity.paste(border_layer_light, (0, 0), rim_mask)

    # Render black motif
    black_motif = Image.new('RGBA', (target_width, target_height), (9, 9, 11, 255))
    black_motif.putalpha(scaled_motif_alpha)
    light_entity.paste(black_motif, (pos_x, pos_y), scaled_motif_alpha)

    # Composite light icon with shadow
    light_icon = Image.alpha_composite(shadow_layer, light_entity)
    light_icon.save('public/app-icon-light.png', 'PNG')
    print('Generated public/app-icon-light.png (Light mode: White bg, Black motif)')

    # --- 6. Generate DARK MODE App Icon (Black background, White motif) ---
    # Create deep obsidian dark background with subtle gradient
    dark_bg = Image.new('RGBA', (1024, 1024), (0, 0, 0, 0))
    for y in range(1024):
        # Subtle gradient from #131418 at top to #09090b at bottom
        factor = max(0.0, min(1.0, (y - 100) / 824.0))
        r = int(19 - factor * 10) # 19 down to 9
        g = int(20 - factor * 11) # 20 down to 9
        b = int(24 - factor * 13) # 24 down to 11
        for x in range(100, 925):
            dark_bg.putpixel((x, y), (r, g, b, 255))
    
    # Apply squircle mask to background
    dark_entity = Image.new('RGBA', (1024, 1024), (0, 0, 0, 0))
    dark_entity.paste(dark_bg, (0, 0), squircle_mask)

    # Add 1px subtle white/ambient highlight border around dark squircle
    border_layer_dark = Image.new('RGBA', (1024, 1024), (255, 255, 255, 32)) # rgba(255, 255, 255, 0.125)
    dark_entity.paste(border_layer_dark, (0, 0), rim_mask)

    # Render white motif
    white_motif = Image.new('RGBA', (target_width, target_height), (255, 255, 255, 255))
    white_motif.putalpha(scaled_motif_alpha)
    dark_entity.paste(white_motif, (pos_x, pos_y), scaled_motif_alpha)

    # Composite dark icon with shadow
    dark_icon = Image.alpha_composite(shadow_layer, dark_entity)
    dark_icon.save('public/app-icon-dark.png', 'PNG')
    print('Generated public/app-icon-dark.png (Dark mode: Black bg, White motif)')

    # --- 7. Save default app-icon.png (using dark mode by default as official system icon) ---
    dark_icon.save('public/app-icon.png', 'PNG')
    print('Updated public/app-icon.png')

if __name__ == '__main__':
    create_desktop_icons()
