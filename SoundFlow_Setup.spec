# -*- mode: python ; coding: utf-8 -*-


a = Analysis(
    ['C:/flowproject/installer/setup_gui.py'],
    pathex=[],
    binaries=[],
    datas=[('C:/flowproject/installer/soundflow_payload.zip', '.'), ('C:/flowproject/assets/icon.ico', 'assets'), ('C:/flowproject/installer/MicrosoftEdgeWebview2Setup.exe', '.')],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='SoundFlow_Setup',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=['C:/flowproject/assets/icon.ico'],
)
