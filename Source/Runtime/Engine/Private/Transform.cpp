#include "Transform.h"
#include "Scene.h"

Matrix4 Transform::GetWorldMatrix()
{
	if (!isMatUpdated)
	{
		mat = Creator::Matrix::FromTRS(position, rotation, scale);
		isMatUpdated = true;
	}

	if (parent != Entity::IdNone && !parentTransform)
		SetParentTransform();

	const Matrix4 parentMat = parentTransform ?
		parentTransform->GetWorldMatrix() : Matrix4::Identity;

	return parentMat * mat;
}

void Transform::SetParent(uint32 inParent)
{
	if (parent == inParent)
		return;

	parent = inParent;
	SetParentTransform();
}

void Transform::Serialize(Json& json)
{
	json["position"] = { position.x, position.y, position.z };
	json["rotation"] = { rotation.roll, rotation.pitch, rotation.yaw };
	json["scale"] = { scale.x, scale.y, scale.z };

	if (parent != Entity::IdNone)
		json["parent"] = parent;
}

void Transform::Deserialize(const Json& json)
{
	const auto inPos = json["position"].get<std::vector<float>>();
	SetPosition(Vector3{ inPos.data() });

	const auto inRot = json["rotation"].get<std::vector<float>>();
	SetRotation(Rotator{ inRot.data() });

	const auto inScale = json["scale"].get<std::vector<float>>();
	SetScale(Vector3{ inScale.data() });

	if (json.contains("parent"))
		SetParent(json["parent"].get<uint32>());
}

void Transform::SetParentTransform()
{
	if (parentTransform)
	{
		parentTransform->GetEntity()->OnChangeId -=
			Delegate<void, uint32, uint32>{ this, &Transform::OnChangeParentId };
	}

	if (parent == Entity::IdNone)
	{
		parentTransform = nullptr;
		return;
	}

	if (const auto entity = GetEntity()->GetScene()->GetEntity(parent))
	{
		parentTransform = entity->GetTransform();
		parentTransform->GetEntity()->OnChangeId +=
			Delegate<void, uint32, uint32>{ this, & Transform::OnChangeParentId };
	}
	else
		parentTransform = nullptr;
}
