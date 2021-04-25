# Source/Runtime/Core

register_library ()

target_link_libraries (Core PUBLIC BSBase::BSBase BSMath::BSMath fmt::fmt-header-only PRIVATE nlohmann_json::nlohmann_json spdlog::spdlog utf8cpp)