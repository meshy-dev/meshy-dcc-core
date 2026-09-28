# ruff: noqa: F821

name = "asset_material_authoring"
version = "1.0.0"
requires = ["meshy_dcc_core"]


def commands():
    env.MESHY_DCC_CONTEXT_BUNDLE = "asset.materials.fabric-library.authoring"
    env.MESHY_DCC_PRODUCTION_DOMAIN = "asset"
    env.MESHY_DCC_CONTEXT_KIND = "asset"
    env.MESHY_DCC_PROJECT = "shared-assets"
    env.MESHY_DCC_ASSET = "fabric-library"
    env.MESHY_DCC_ASSET_TYPE = "material"
    env.MESHY_DCC_TASK = "material-authoring"
    env.MESHY_DCC_TOOLSET_PROFILE = "asset-material-authoring"
    env.MESHY_DCC_PACKAGE_PROVENANCE.append("asset_material_authoring-1.0.0")
    env.MESHY_DCC_SKILL_PATHS.append("{root}/skills")
