#include "RenderManager.h"
#include <algorithm>
#include <exception>
#include <GL/glew.h>
#include <GLFW/glfw3.h>
#include <string>
#include "DrawableComponent.h"
#include "MathFunctions.h"
#include "Matrix4x4.h"
#include "MeshComponent.h"
#include "RenderTree.h"
#include "ResourceManager.h"
#include "Shader.h"
#include "SpriteComponent.h"
#include "VertexArray.h"
#include "WindowManager.h"

namespace ArenaBoss
{
	RenderManager::RenderManager()
		: spriteComponents(),
		renderTree(new RenderTree{}),
		view(),
		projection()
	{
		glewExperimental = GL_TRUE;
		if (glewInit() != GLEW_OK)
			throw std::exception{ "Failed to initialize GLEW" };

		glClearColor(0.0f, 0.0f, 0.0f, 1.0f);
		GenerateSpriteResource();
	}

	RenderManager::~RenderManager()
	{
		delete renderTree;
	}

	void RenderManager::Draw()
	{
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

		glDisable(GL_BLEND);
		glEnable(GL_DEPTH_TEST);

		renderTree->Draw([this, viewProj = view * projection](auto& shader)
		{
			shader.SetUniformValue("uViewProjection", viewProj);
			SetLightUniforms(shader);
		});

		glDisable(GL_DEPTH_TEST);
		glEnable(GL_BLEND);

		glBlendEquationSeparate(GL_FUNC_ADD, GL_FUNC_ADD);
		glBlendFuncSeparate(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ZERO);

		auto& resourceManager = ResourceAccessor::GetManager();
		resourceManager.GetResource<Shader>("Sprite Shader")->Activate();
		resourceManager.GetResource<VertexArray>("Sprite Vertex")->Activate();

		for (auto component : spriteComponents)
			if (component->IsVisible())
				component->Draw();

		// Draw ui

		WindowAccessor::GetManager().SwapBuffer();
	}

	void RenderManager::SetComponentInTree(MeshComponent* component)
	{
		if (component->HaveShader())
			renderTree->RegisterComponent(component);
		else
			renderTree->UnregisterComponent(component);
	}

	void RenderManager::RegisterComponent(MeshComponent* component)
	{
		meshComponents.push_back(component);

		if (component->HaveShader())
			renderTree->RegisterComponent(component);
	}

	void RenderManager::UnregisterComponent(MeshComponent* component)
	{
		const auto iter = std::find(meshComponents.begin(),
			meshComponents.end(), component);
		meshComponents.erase(iter);
	}

	void RenderManager::UnregisterComponent(SpriteComponent* component)
	{
		const auto iter = std::find(spriteComponents.begin(),
			spriteComponents.end(), component);
		spriteComponents.erase(iter);
	}

	void RenderManager::GenerateSpriteResource()
	{
		auto& windowManager = WindowAccessor::GetManager();
		auto& resourceManager = ResourceAccessor::GetManager();

		view = Math::Matrix4x4::CreateLookAt(Math::Vector3::ZERO(), Math::Vector3::BACKWARD(), Math::Vector3::UP());

		const auto width = windowManager.GetWidth();
		const auto height = windowManager.GetHeight();

		projection = Math::Matrix4x4::CreateOrtho(static_cast<float>(width),
			static_cast<float>(height), 25.0f, 10000.0f);

		auto shader = resourceManager.CreateResource<Shader>
			("Sprite Shader", "Sprite.vert", "Sprite.frag");

		shader->Activate();

		const Math::Matrix4x4 viewProjection =
			Math::Matrix4x4::CreateSimpleViewProjection(static_cast<float>(width), static_cast<float>(height));

		shader->SetUniformValue("uViewProjection", viewProjection);

		constexpr float vertices[]
		{
			-0.5f, 0.5f, 0.f, 0.f, 0.f, 0.0f, 0.f, 0.f,
			0.5f, 0.5f, 0.f, 0.f, 0.f, 0.0f, 1.f, 0.f,
			0.5f, -0.5f, 0.f, 0.f, 0.f, 0.0f, 1.f, 1.f,
			-0.5f, -0.5f, 0.f, 0.f, 0.f, 0.0f, 0.f, 1.f
		};

		constexpr uint indices[]
		{
			0, 1, 2,
			2, 3, 0
		};

		VertexArrayParam param{ VertexLayout::PosNormTex, vertices, 4, indices, 6 };
		resourceManager.CreateResource<VertexArray>("Sprite Vertex", std::move(param));
	}

	void RenderManager::SetLightUniforms(Shader& shader)
	{
		auto invView = view.Invert();

		shader.SetUniformValue("uCameraPos", invView.GetTranslation());

		shader.SetUniformValue("uAmbientLight", ambientLight);

		shader.SetUniformValue("uDirLight.direction", dirLight.direction);
		shader.SetUniformValue("uDirLight.diffuseColor", dirLight.diffuseColor);
		shader.SetUniformValue("uDirLight.specularColor", dirLight.specularColor);
	}
}