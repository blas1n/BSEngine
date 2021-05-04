#pragma once

#include "Component.h"
#include "BSMath.h"
#include <vector>

class ENGINE_API Transform final : public Component
{
public:
	using Super = Component;
	using Super::Super;

	[[nodiscard]] Matrix4 GetWorldMatrix();

	void Serialize(Json& json) override;
	void Deserialize(const Json& json) override;

	[[nodiscard]] uint32 GetParent() const noexcept { return parent; }
	void SetParent(uint32 inParent);

	[[nodiscard]] const Vector3& GetPosition() const noexcept { return position; }
	[[nodiscard]] const Rotator& GetRotation() const noexcept { return rotation; }
	[[nodiscard]] const Vector3& GetScale() const noexcept { return scale; }

	void SetPosition(const Vector3& inPos) noexcept { position = inPos; isMatUpdated = false; }
	void SetRotation(const Rotator& inRot) noexcept { rotation = inRot; isMatUpdated = false; }
	void SetScale(const Vector3& inScale) noexcept { scale = inScale; isMatUpdated = false; }

private:
	void SetParentTransform();
	void OnChangeParentId(uint32 newId, uint32 oldId) { parent = newId; }

private:
	constexpr static uint32 ParentNone = static_cast<uint32>(-1);

	Matrix4 mat;

	Vector3 position;
	Rotator rotation;
	Vector3 scale;

	Transform* parentTransform;
	uint32 parent = ParentNone;

	uint8 isMatUpdated : 1;
};
