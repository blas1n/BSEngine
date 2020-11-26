#include "JsonHelper.h"
#include "Rotator.h"

namespace ArenaBoss::Json
{
	std::optional<int> JsonHelper::GetInt(const Object& inObject, const char* name)
	{
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		const auto& property = iter->value;
		if (!property.IsInt())
			return {};

		return property.GetInt();
	}

	std::optional<float> JsonHelper::GetFloat(const Object& inObject, const char* name)
	{
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		const auto& property = iter->value;
		if (!property.IsFloat())
			return {};

		return property.GetFloat();
	}

	std::optional<std::string> JsonHelper::GetString(const Object& inObject, const char* name) {
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		const auto& property = iter->value;
		if (!property.IsString())
			return {};

		return property.GetString();
	}

	std::optional<bool> JsonHelper::GetBool(const Object& inObject, const char* name)
	{
		const auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		const auto& property = iter->value;
		if (!property.IsBool())
			return {};

		return property.GetBool();
	}

	std::optional<Math::Vector2> JsonHelper::GetVector2(const Object& inObject, const char* name)
	{
		auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		auto& property = iter->value;
		if (!property.IsArray() || property.Size() != 2)
			return {};

		for (rapidjson::SizeType i = 0; i < 2; i++)
			if (!property[i].IsFloat())
				return {};

		return Math::Vector2
		{
			property[0].GetFloat(),
			property[1].GetFloat()
		};
	}

	std::optional<Math::Vector3> JsonHelper::GetVector3(const Object& inObject, const char* name)
	{
		auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		auto& property = iter->value;
		if (!property.IsArray() || property.Size() != 3)
			return {};

		for (rapidjson::SizeType i = 0; i < 3; i++)
			if (!property[i].IsFloat())
				return {};

		return Math::Vector3
		{
			property[0].GetFloat(),
			property[1].GetFloat(),
			property[2].GetFloat()
		};
	}

	std::optional<Math::Rotator> JsonHelper::GetRotator(const Object& inObject, const char* name)
	{
		auto iter = inObject.FindMember(name);
		if (iter == inObject.MemberEnd())
			return {};

		auto& property = iter->value;
		if (!property.IsArray() || property.Size() != 3)
			return {};

		for (rapidjson::SizeType i = 0; i < 3; i++)
			if (!property[i].IsFloat())
				return {};

		return Math::Rotator
		{
			property[0].GetFloat(),
			property[1].GetFloat(),
			property[2].GetFloat()
		};
	}

	void JsonHelper::AddInt(JsonSaver& saver, const char* name, int value)
	{
		Object v{ value };
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void JsonHelper::AddFloat(JsonSaver& saver, const char* name, float value)
	{
		Object v{ value };
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void JsonHelper::AddString(JsonSaver& saver, const char* name, const std::string& value)
	{
		Object v;
		v.SetString(value.c_str(), static_cast<rapidjson::SizeType>(value.length()), saver.alloc);
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void JsonHelper::AddBool(JsonSaver& saver, const char* name, const bool value)
	{
		Object v{ value };
		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void JsonHelper::AddVector2(JsonSaver& saver, const char* name, const Math::Vector2& value)
	{
		Object v{ rapidjson::kArrayType };

		v.PushBack(Object{ value.x }.Move(), saver.alloc);
		v.PushBack(Object{ value.y }.Move(), saver.alloc);

		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void JsonHelper::AddVector3(JsonSaver& saver, const char* name, const Math::Vector3& value)
	{
		Object v{ rapidjson::kArrayType };

		v.PushBack(Object{ value.x }.Move(), saver.alloc);
		v.PushBack(Object{ value.y }.Move(), saver.alloc);
		v.PushBack(Object{ value.z }.Move(), saver.alloc);

		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}

	void JsonHelper::AddRotator(JsonSaver& saver, const char* name, const Math::Rotator& value)
	{
		Object v{ rapidjson::kArrayType };

		v.PushBack(Object{ value.roll }.Move(), saver.alloc);
		v.PushBack(Object{ value.pitch }.Move(), saver.alloc);
		v.PushBack(Object{ value.yaw }.Move(), saver.alloc);

		saver.object.AddMember(rapidjson::StringRef(name), v, saver.alloc);
	}
}