#include "Transform.h"
#include "Scene.h"
#include "SceneManager.h"

REGISTER_COMPONENT(Transform)

Matrix4 Transform::GetWorldMatrix()
{
	if (!isUpdated)
	{
		mat = Creator::Matrix::FromTRS(position, rotation, scale);
		isUpdated = true;
	}

	if (!parentName.empty() && !parent)
		SetParentTransform();

	const Matrix4 parentMat = parent ?
		parent->GetWorldMatrix() : Matrix4::Identity;

	return parentMat * mat;
}

Json Transform::Serialize() const
{
	Json json = Json::object();

	json["position"] = { position.x, position.y, position.z };
	json["rotation"] = { rotation.roll, rotation.pitch, rotation.yaw };
	json["scale"] = { scale.x, scale.y, scale.z };

	if (parent)
		json["parent"] = parent->GetEntity()->GetName();

	return json;
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
		parentName = json["parent"].get<String>();
}

void Transform::SetParentTransform()
{
	const auto entity = Accessor<SceneManager>::GetManager()->GetScene().GetEntity(parentName);
	if (entity)
		parent = entity->GetComponent<Transform>();
}
