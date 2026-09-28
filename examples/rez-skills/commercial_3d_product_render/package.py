# ruff: noqa: F821

name = "commercial_3d_product_render"
version = "1.0.0"
requires = ["meshy_dcc_core", "meshy_dcc_blender"]


def commands():
    env.MESHY_DCC_CONTEXT_BUNDLE = "product.launch-pack.hero-render.lookdev"
    env.MESHY_DCC_PRODUCTION_DOMAIN = "advertising"
    env.MESHY_DCC_CONTEXT_KIND = "product"
    env.MESHY_DCC_PROJECT = "launch-pack"
    env.MESHY_DCC_ASSET = "hero-product"
    env.MESHY_DCC_TASK = "product-render"
    env.MESHY_DCC_TOOLSET_PROFILE = "commercial-3d-product-render"
    env.MESHY_DCC_PACKAGE_PROVENANCE.append("commercial_3d_product_render-1.0.0")
    env.MESHY_DCC_BLENDER_SKILL_PATHS.append("{root}/skills")
