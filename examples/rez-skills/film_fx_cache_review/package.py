# ruff: noqa: F821

name = "film_fx_cache_review"
version = "1.0.0"
requires = ["meshy_dcc_core", "meshy_dcc_houdini"]


def commands():
    env.MESHY_DCC_CONTEXT_BUNDLE = "show-a.seq010.shot020.fx"
    env.MESHY_DCC_PRODUCTION_DOMAIN = "film"
    env.MESHY_DCC_CONTEXT_KIND = "shot"
    env.MESHY_DCC_PROJECT = "show-a"
    env.MESHY_DCC_SEQUENCE = "seq010"
    env.MESHY_DCC_SHOT = "shot020"
    env.MESHY_DCC_TASK = "fx"
    env.MESHY_DCC_TOOLSET_PROFILE = "film-shot-fx"
    env.MESHY_DCC_PACKAGE_PROVENANCE.append("film_fx_cache_review-1.0.0")
    env.MESHY_DCC_HOUDINI_SKILL_PATHS.append("{root}/skills")
