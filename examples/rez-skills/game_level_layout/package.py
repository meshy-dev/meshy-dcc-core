# ruff: noqa: F821

name = "game_level_layout"
version = "1.0.0"
requires = ["meshy_dcc_core", "meshy_dcc_unreal"]


def commands():
    env.MESHY_DCC_CONTEXT_BUNDLE = "game-demo.level.city-block.layout"
    env.MESHY_DCC_PRODUCTION_DOMAIN = "game"
    env.MESHY_DCC_CONTEXT_KIND = "level"
    env.MESHY_DCC_PROJECT = "game-demo"
    env.MESHY_DCC_ASSET = "city-block"
    env.MESHY_DCC_TASK = "level-layout"
    env.MESHY_DCC_TOOLSET_PROFILE = "game-level-layout"
    env.MESHY_DCC_PACKAGE_PROVENANCE.append("game_level_layout-1.0.0")
    env.MESHY_DCC_UNREAL_SKILL_PATHS.append("{root}/skills")
