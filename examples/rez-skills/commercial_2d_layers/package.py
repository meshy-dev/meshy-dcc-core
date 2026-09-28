# ruff: noqa: F821

name = "commercial_2d_layers"
version = "1.0.0"
requires = ["meshy_dcc_core", "meshy_dcc_photoshop"]


def commands():
    env.MESHY_DCC_CONTEXT_BUNDLE = "ad-spot.deliverable.social-16x9.motion"
    env.MESHY_DCC_PRODUCTION_DOMAIN = "advertising"
    env.MESHY_DCC_CONTEXT_KIND = "deliverable"
    env.MESHY_DCC_PROJECT = "ad-spot"
    env.MESHY_DCC_TASK = "motion-2d"
    env.MESHY_DCC_TOOLSET_PROFILE = "commercial-2d-motion"
    env.MESHY_DCC_PACKAGE_PROVENANCE.append("commercial_2d_layers-1.0.0")
    env.MESHY_DCC_PHOTOSHOP_SKILL_PATHS.append("{root}/skills")
