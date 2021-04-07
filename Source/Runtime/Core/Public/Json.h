#pragma once

#include <optional>
#include "BSMath/Basic.h"
#include "rapidjson/document.h"
#include "CharSet.h"

namespace Json
{
	using Object = rapidjson::GenericValue<rapidjson::UTF16<Char>>;

	struct CORE_API JsonSaver final
	{
		using Allocator = rapidjson::Document::AllocatorType;

		JsonSaver(Allocator& inAlloc, Object& inObject)
			: alloc(inAlloc), object(inObject) {}

		JsonSaver(JsonSaver& other, Object& inObject)
			: alloc(other.alloc), object(inObject) {}

		Allocator& alloc;
		Object& object;
	};

	[[nodiscard]] CORE_API std::optional<int> GetInt(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<float> GetFloat(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<String> GetString(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<bool> GetBool(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<Vector2> GetVector2(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<Vector3> GetVector3(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<Vector4> GetVector4(const Object& object, const Char* name);
	[[nodiscard]] CORE_API std::optional<Rotator> GetRotator(const Object& object, const Char* name);

	CORE_API void AddInt(JsonSaver& saver, const Char* name, int value);
	CORE_API void AddFloat(JsonSaver& saver, const Char* name, float value);
	CORE_API void AddString(JsonSaver& saver, const Char* name, const String& value);
	CORE_API void AddBool(JsonSaver& saver, const Char* name, bool value);
	CORE_API void AddVector2(JsonSaver& saver, const Char* name, const Vector2& value);
	CORE_API void AddVector3(JsonSaver& saver, const Char* name, const Vector3& value);
	CORE_API void AddVector4(JsonSaver& saver, const Char* name, const Vector4& value);
	CORE_API void AddRotator(JsonSaver& saver, const Char* name, const Rotator& value);
}
