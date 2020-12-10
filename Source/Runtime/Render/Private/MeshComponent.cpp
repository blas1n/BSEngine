#include "MeshComponent.h"
#include "Entity.h"
#include "JsonHelper.h"
#include "Mesh.h"
#include "RenderManager.h"
#include "ResourceManager.h"
#include "Shader.h"
#include "Texture.h"
#include "Transform.h"
#include "VertexArray.h"

namespace ArenaBoss
{
	void MeshComponent::Load(const Json::Object& object)
	{
		Super::Load(object);
		
		auto& manager = Accessor<ResourceManager>::GetManager();

		if (object.HasMember("shader"))
		{
			const auto& shaderObj = object["shader"];
			const auto shaderName = *Json::JsonHelper::GetString(shaderObj, "name");
			const auto shaderVert = *Json::JsonHelper::GetString(shaderObj, "vert");
			const auto shaderFrag = *Json::JsonHelper::GetString(shaderObj, "frag");

			SetShader(manager.CreateResource<Shader>(shaderName, shaderVert, shaderFrag));
		}

		if (object.HasMember("mesh"))
		{
			const auto& meshObj = object["mesh"];
			const auto meshName = *Json::JsonHelper::GetString(meshObj, "name");
			const auto meshPath = *Json::JsonHelper::GetString(meshObj, "path");

			mesh = manager.CreateResource<Mesh>(meshName, meshPath);
		}
		
		textureIndex = *Json::JsonHelper::GetInt(object, "texture index");
	}

	void MeshComponent::Save(Json::JsonSaver& saver) const
	{
		Super::Save(saver);

		if (const auto* shader = Super::GetShader())
		{
			rapidjson::Value obj{ rapidjson::kObjectType };
			Json::JsonSaver shaderSaver{ saver, obj };

			Json::JsonHelper::AddString(shaderSaver, "name", mesh->GetName());
			Json::JsonHelper::AddString(shaderSaver, "vert", mesh->GetPath());
			Json::JsonHelper::AddString(shaderSaver, "frag", mesh->GetPath());

			saver.object.AddMember("shader", obj, saver.alloc);
		}

		if (mesh)
		{
			rapidjson::Value obj{ rapidjson::kObjectType };
			Json::JsonSaver meshSaver{ saver, obj };

			Json::JsonHelper::AddString(meshSaver, "name", mesh->GetName());
			Json::JsonHelper::AddString(meshSaver, "path", mesh->GetPath());

			saver.object.AddMember("mesh", obj, saver.alloc);
		}

		Json::JsonHelper::AddInt(saver, "texture index", static_cast<int>(textureIndex));
	}

	void MeshComponent::Draw()
	{
		if (!mesh) return;

		auto* shader = GetShader();

		shader->SetUniformValue("uWorldTransform", GetEntity()->GetComponent<Transform>().GetWorldMatrix());
		shader->SetUniformValue("uSpecularPower", mesh->GetSpecularPower());

		auto t = mesh->GetTexture(textureIndex);
		if (t) t->Activate();

		auto vertexArray = mesh->GetVertexArray();
		vertexArray->Activate();

		glDrawElements(GL_TRIANGLES, vertexArray->GetNumIndices(), GL_UNSIGNED_INT, nullptr);
	}

	void MeshComponent::SetMesh(Mesh* inMesh) noexcept
	{
		if (mesh) delete mesh;
		mesh = inMesh;
	}

	void MeshComponent::SetShader(Shader* inShader) noexcept
	{
		DrawableComponent::SetShader(inShader);
		Accessor<RenderManager>::GetManager().SetComponentInTree(this);
	}
}