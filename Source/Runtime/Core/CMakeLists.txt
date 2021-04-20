# Source/Runtime/Core

register_library ()

target_link_libraries (Core PUBLIC BSBase::BSBase BSMath::BSMath fmt::fmt-header-only PRIVATE spdlog::spdlog utf8cpp)
target_include_directories (Core PUBLIC ${RAPIDJSON_INCLUDE_DIRS})