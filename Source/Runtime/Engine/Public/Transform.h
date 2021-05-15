#pragma once

#include "Accessor.h"
#include "Component.h"
#include <vector>
#include "BSMath.h"

class ENGINE_API Transform final : public Component, public Accessor<class SceneManager>
{
public:
	using Component::Component;

	[[nodiscard]] Matrix4 GetWorldMatrix();

	[[nodiscard]] Json Serialize() const override;
	void Deserialize(const Json& json) override;

	[[nodiscard]] Transform* GetParent() noexcept { return parent; }
	[[nodiscard]] const Transform* GetParent() const noexcept { return parent; }

	void SetParent(Transform* inParent) noexcept { parent = inParent; }

	[[nodiscard]] const Vector3& GetPosition() const noexcept { return position; }
	[[nodiscard]] const Rotator& GetRotation() const noexcept { return rotation; }
	[[nodiscard]] const Vector3& GetScale() const noexcept { return scale; }

	void SetPosition(const Vector3& inPos) noexcept { position = inPos; isUpdated = false; }
	void SetRotation(const Rotator& inRot) noexcept { rotation = inRot; isUpdated = false; }
	void SetScale(const Vector3& inScale) noexcept { scale = inScale; isUpdated = false; }

private:
	void SetParentTransform();

private:
	Matrix4 mat;

	Vector3 position;
	Rotator rotation;
	Vector3 scale;

	Transform* parent;
	String parentName;

	uint8 isUpdated : 1;
};
