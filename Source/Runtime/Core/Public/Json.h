#pragma once

#include "nlohmann/json.hpp"
#include "BSBase/Type.h"
#include "CharSet.h"

using Json = nlohmann::basic_json<std::map, std::vector,
		String, bool, BSBase::int64, BSBase::uint64, float,
		std::allocator, nlohmann::adl_serializer,
		std::vector<BSBase::uint8, std::allocator<BSBase::uint8>>>;

using JsonValue = nlohmann::detail::value_t;
