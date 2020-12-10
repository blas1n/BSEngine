#pragma once

#include "Component.h"
#include "Vector3.h"

namespace ArenaBoss
{
	namespace Math
	{
		class Rotator;
		class Matrix4x4;
	}

	class Transform : public Component
	{
		GENERATE_COMPONENT1(Transform)

	public:
		void Construct() override;
		void Destruct() noexcept override;

		void Load(const Json::Object& object) override;
		void Save(Json::JsonSaver& saver) const override;

		const Math::Vector3& GetPosition() const noexcept;
		const Math::Rotator& GetRotation() const noexcept;
		float GetScale() const noexcept;

		void SetPosition(const Math::Vector3& position) noexcept;
		void SetRotation(const Math::Rotator& rotation) noexcept;
		void SetScale(float scale) noexcept;

		const Math::Matrix4x4& GetWorldMatrix();

	private:
		struct TransformImpl* impl = nullptr;
	};
}