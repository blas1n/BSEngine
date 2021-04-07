#include "Json.h"
#include "Math.h"

namespace Json
{
	std::optional<int> GetInt(const Object& inObject, const Char* name)
	{
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		const auto& property = iter->value;
		if (!property.IsInt())
			return std::nullopt;

		return property.GetInt();
	}

	std::optional<float> GetFloat(const Object& inObject, const Char* name)
	{
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		const auto& property = iter->value;
		if (!property.IsFloat())
			return std::nullopt;

		return property.GetFloat();
	}

	std::optional<String> GetString(const Object& inObject, const Char* name) {
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		const auto& property = iter->value;
		if (!property.IsString())
			return std::nullopt;

		return String{ property.GetString() };
	}

	std::optional<bool> GetBool(const Object& inObject, const Char* name)
	{
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		const auto& property = iter->value;
		if (!property.IsBool())
			return std::nullopt;

		return property.GetBool();
	}

	std::optional<Vector2> GetVector2(const Object& inObject, const Char* name)
	{
		auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		auto& property = iter->value;
		if (!property.IsArray() || property.Size() != 2)
			return std::nullopt;

		for (rapidjson::SizeType i = 0; i < 2; i++)
			if (!property[i].IsFloat())
				return std::nullopt;

		return Vector2
		{
			property[0].GetFloat(),
			property[1].GetFloat()
		};
	}

	std::optional<Vector3> GetVector3(const Object& inObject, const Char* name)
	{
		auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		auto& property = iter->value;
		if (!property.IsArray() || property.Size() != 3)
			return std::nullopt;

		for (rapidjson::SizeType i = 0; i < 3; i++)
			if (!property[i].IsFloat())
				return std::nullopt;

		return Vector3
		{
			property[0].GetFloat(),
			property[1].GetFloat(),
			property[2].GetFloat()
		};
	}

	std::optional<Vector4> GetVector4(const Object& inObject, const Char* name)
	{
		auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return std::nullopt;

		auto& property = iter->value;
		if (!property.IsArray() || property.Size() != 4)
			return std::nullopt;

		for (rapidjson::SizeType i = 0; i < 4; i++)
			if (!property[i].IsFloat())
				return std::nullopt;

		return Vector4
		{
			property[0].GetFloat(),
			property[1].GetFloat(),
			property[2].GetFloat(),
			property[3].GetFloat()
		};
	}

	std::optional<Rotator> GetRotator(const Object& inObject, const Char* name)
	{
		const auto vec = GetVector3(inObject, name);
		if (vec)
			return Creator::Rotator::FromEuler(*vec);

		return std::nullopt;
	}

	void AddInt(JsonSaver& saver, const Char* name, int value)
	{
		Object v{ value };
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddFloat(JsonSaver& saver, const Char* name, float value)
	{
		Object v{ value };
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddString(JsonSaver& saver, const Char* name, const String& value)
	{
		Object v;
		v.SetString(value.c_str(), static_cast<rapidjson::SizeType>(value.length()), saver.alloc);
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddBool(JsonSaver& saver, const Char* name, const bool value)
	{
		Object v{ value };
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddVector2(JsonSaver& saver, const Char* name, const Vector2& value)
	{
		Object v{ rapidjson::kArrayType };

		v.PushBack(Object{ value.x }.Move(), saver.alloc);
		v.PushBack(Object{ value.y }.Move(), saver.alloc);

		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddVector3(JsonSaver& saver, const Char* name, const Vector3& value)
	{
		Object v{ rapidjson::kArrayType };

		v.PushBack(Object{ value.x }.Move(), saver.alloc);
		v.PushBack(Object{ value.y }.Move(), saver.alloc);
		v.PushBack(Object{ value.z }.Move(), saver.alloc);

		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddVector4(JsonSaver& saver, const Char* name, const Vector4& value)
	{
		Object v{ rapidjson::kArrayType };

		v.PushBack(Object{ value.x }.Move(), saver.alloc);
		v.PushBack(Object{ value.y }.Move(), saver.alloc);
		v.PushBack(Object{ value.z }.Move(), saver.alloc);
		v.PushBack(Object{ value.w }.Move(), saver.alloc);

		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void AddRotator(JsonSaver& saver, const Char* name, const Rotator& value)
	{
		return AddVector3(saver, name, Vector3{ value.roll, value.pitch, value.yaw });
	}
}
