#pragma once

#include <rapidjson/document.h>

namespace ArenaBoss::Json
{
	using Allocator = rapidjson::Document::AllocatorType;
	using Object = rapidjson::Value;
	struct JsonSaver;
}