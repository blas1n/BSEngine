#pragma once

#include "Component.h"
#include "BSMath.h"
#include <vector>

namespace Impl
{
	struct Node final
	{
		Transform* ptr;
		uint32 id;
	};
}

class ENGINE_API Transform final : public Component
{
public:
	using Super = Component;
	using Super::Super;

	Matrix4 GetWorldMatrix() const;

	void SetParent(uint32 inParent);
	void AddChild(uint32 inChild);

	void Serialize(Json& json) override;
	void Deserialize(const Json& json) override;

	const Vector3& GetPosition() const noexcept { return position; }
	const Rotator& GetRotation() const noexcept { return rotation; }
	const Vector3& GetScale() const noexcept { return scale; }

	void SetPosition(const Vector3& inPos) noexcept { position = inPos; isMatUpdated = false; }
	void SetRotation(const Rotator& inRot) noexcept { rotation = inRot; isMatUpdated = false; }
	void SetScale(const Vector3& inScale) noexcept { scale = inScale; isMatUpdated = false; }

private:
	void SetParentImpl(Impl::Node inParent) { parent = inParent; }
	void AddChildImpl(Impl::Node inParent) { children.push_back(inParent); }

private:
	mutable Matrix4 mat;

	Vector3 position;
	Rotator rotation;
	Vector3 scale;

	std::vector<Impl::Node> children;
	Impl::Node parent;

	mutable uint8 isMatUpdated : 1;
};
