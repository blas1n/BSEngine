#include "Transform.h"
#include "Entity.h"
#include "Scene.h"

Matrix4 Transform::GetWorldMatrix() const
{
	if (!isMatUpdated)
	{
		mat = Creator::Matrix::FromTRS(position, rotation, scale);
		isMatUpdated = true;
	}

	return parent.ptr->GetWorldMatrix() * mat;
}

void Transform::SetParent(uint32 inParent)
{
	const auto parentTransform = GetEntity()->
		GetScene()->GetEntity(inParent)->GetTransform();

	const auto myId = GetEntity()->GetId();

	SetParentImpl(Impl::Node{ parentTransform, inParent });
	parentTransform->AddChildImpl(Impl::Node{ this, myId });
}

void Transform::AddChild(uint32 inChild)
{
	const auto childTransform = GetEntity()->
		GetScene()->GetEntity(inChild)->GetTransform();

	const auto myId = GetEntity()->GetId();

	AddChildImpl(Impl::Node{ childTransform, inChild });
	childTransform->SetParentImpl(Impl::Node{ this, myId });
}

void Transform::Serialize(Json& json)
{
	json["position"] = { position.x, position.y, position.z };
	json["rotation"] = { rotation.roll, rotation.pitch, rotation.yaw };
	json["scale"] = { scale.x, scale.y, scale.z };

	json["parent"] = parent.id;

	const size_t childNum = children.size();
	auto& childList = json["children"];
	childList = Json::array();

	for (size_t i = 0; i < childNum; ++i)
		json["children"].push_back(children[i].id);
}

void Transform::Deserialize(const Json& json)
{
	const auto inPos = json["position"].get<std::vector<float>>();
	SetPosition(Vector3{ inPos.data() });

	const auto inRot = json["rotation"].get<std::vector<float>>();
	SetRotation(Rotator{ inRot.data() });

	const auto inScale = json["scale"].get<std::vector<float>>();
	SetScale(Vector3{ inScale.data() });

	/// @todo : How to connect to an object that has not yet been created
}

void Transform::SetParentImpl(Impl::Node inParent)
{
	parent = inParent;
}

void Transform::AddChildImpl(Impl::Node inChild)
{
	children.emplace_back(inChild);
}
