#include "Transform.h"
#include "JsonHelper.h"
#include "Matrix4x4.h"
#include "Rotator.h"

namespace ArenaBoss
{
	struct TransformImpl
	{
		Math::Vector3 position;
		Math::Rotator rotation;
		float scale = 1.0f;

		Math::Matrix4x4 worldMatrix;
		bool needRecompute = true;
	};

	void Transform::Construct()
	{
		Super::Construct();

		impl = new TransformImpl{};
	}

	void Transform::Destruct() noexcept
	{
		delete impl;

		Super::Destruct();
	}

	void Transform::Load(const Json::Object& object)
	{
		Super::Load(object);

		impl->position = *Json::JsonHelper::GetVector3(object, "position");
		impl->rotation = *Json::JsonHelper::GetRotator(object, "rotation");
		impl->scale = *Json::JsonHelper::GetFloat(object, "scale");
	}

	void Transform::Save(Json::JsonSaver& saver) const
	{
		Super::Save(saver);

		Json::JsonHelper::AddVector3(saver, "position", impl->position);
		Json::JsonHelper::AddRotator(saver, "rotation", impl->rotation);
		Json::JsonHelper::AddFloat(saver, "scale", impl->scale);
	}

	const Math::Vector3& Transform::GetPosition() const noexcept
	{
		return impl->position;
	}

	const Math::Rotator& Transform::GetRotation() const noexcept
	{
		return impl->rotation;
	}

	float Transform::GetScale() const noexcept
	{
		return impl->scale;
	}

	void Transform::SetPosition(const Math::Vector3& position) noexcept
	{
		impl->position = position;
		impl->needRecompute = true;
	}

	void Transform::SetRotation(const Math::Rotator& rotation) noexcept
	{
		impl->rotation = rotation;
		impl->needRecompute = true;
	}

	void Transform::SetScale(float scale) noexcept
	{
		impl->scale = scale;
		impl->needRecompute = true;
	}

	const Math::Matrix4x4& Transform::GetWorldMatrix()
	{
		if (impl->needRecompute)
		{
			impl->worldMatrix = Math::Matrix4x4::CreateFromScale(impl->scale);
			impl->worldMatrix *= Math::Matrix4x4::CreateFromRotation(impl->rotation);
			impl->worldMatrix *= Math::Matrix4x4::CreateFromPosition(impl->position);
			impl->needRecompute = false;
		}

		return impl->worldMatrix;
	}
}