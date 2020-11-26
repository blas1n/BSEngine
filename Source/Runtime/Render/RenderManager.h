#pragma once

#include <vector>
#include "Accessor.h"
#include "Matrix4x4.h"

namespace ArenaBoss
{
	class MeshComponent;
	class SpriteComponent;
	class Shader;
	class RenderTree;

	struct DirectionalLight
	{
		Math::Vector3 direction;
		Math::Vector3 diffuseColor;
		Math::Vector3 specularColor;
	};

	class RenderManager final : Accessor<class WindowManager>, Accessor<class ResourceManager>
	{
	public:
		RenderManager();

		RenderManager(const RenderManager&) = delete;
		RenderManager(RenderManager&&) = delete;

		RenderManager& operator=(const RenderManager&) = delete;
		RenderManager& operator=(RenderManager&&) = delete;

		~RenderManager();

		void Draw();

		void SetComponentInTree(MeshComponent* component);

		inline void SetViewMatrix(const Math::Matrix4x4& inView) noexcept { view = inView; }

		inline const Math::Vector3& GetAmbientLight() const noexcept { return ambientLight; }
		inline void SetAmbientLight(const Math::Vector3& inAmbientLight) noexcept { ambientLight = inAmbientLight; }

		inline DirectionalLight& GetDirectionalLight() noexcept { return dirLight; }
		inline const DirectionalLight& GetDirectionalLight() const noexcept { return dirLight; }

	private:
		friend class ComponentManager;

		void RegisterComponent(MeshComponent* component);

		inline void RegisterComponent(SpriteComponent* component)
		{
			spriteComponents.push_back(component);
		}

		void UnregisterComponent(MeshComponent* component);
		void UnregisterComponent(SpriteComponent* component);

		void GenerateSpriteResource();

		void SetLightUniforms(Shader& shader);

	private:
		using WindowAccessor = Accessor<WindowManager>;
		using ResourceAccessor = Accessor<ResourceManager>;

		std::vector<MeshComponent*> meshComponents;
		std::vector<SpriteComponent*> spriteComponents;

		RenderTree* renderTree;

		Math::Matrix4x4 view;
		Math::Matrix4x4 projection;

		DirectionalLight dirLight;
		Math::Vector3 ambientLight;
	};
}