#pragma once

#include "DrawableComponent.h"

namespace ArenaBoss
{
	class Mesh;

	class MeshComponent : public DrawableComponent, private Accessor<class RenderManager>, private Accessor<class ResourceManager>
	{
		GENERATE_COMPONENT2(MeshComponent, DrawableComponent)

	public:
		void Load(const Json::Object& object) override;
		void Save(Json::JsonSaver& saver) const override;
		
		void Draw() override;

		virtual void SetMesh(Mesh* inMesh) noexcept;
		inline void SetTexutreIndex(size_t inTextureIndex) noexcept { textureIndex = inTextureIndex; }

		void SetShader(Shader* inShader) noexcept override;

	private:
		Mesh* mesh = nullptr;
		size_t textureIndex = 0u;
	};
}