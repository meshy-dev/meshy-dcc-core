# ruff: noqa: F821

name = "film_animation_blocking"
version = "1.0.0"
requires = ["meshy_dcc_core", "meshy_dcc_maya"]


def commands():
    env.MESHY_DCC_CONTEXT_BUNDLE = "show-a.seq010.shot020.animation"
    env.MESHY_DCC_PRODUCTION_DOMAIN = "film"
    env.MESHY_DCC_CONTEXT_KIND = "shot"
    env.MESHY_DCC_PROJECT = "show-a"
    env.MESHY_DCC_SEQUENCE = "seq010"
    env.MESHY_DCC_SHOT = "shot020"
    env.MESHY_DCC_TASK = "animation"
    env.MESHY_DCC_TOOLSET_PROFILE = "film-shot-animation"
    env.MESHY_DCC_PACKAGE_PROVENANCE.append("film_animation_blocking-1.0.0")
    env.MESHY_DCC_MAYA_SKILL_PATHS.append("{root}/skills")
