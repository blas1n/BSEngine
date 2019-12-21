macro (set_project)
	get_filename_component(PROJECT_ID ${CMAKE_CURRENT_SOURCE_DIR} NAME)
	project(${PROJECT_ID})
endmacro ()

macro (set_public_dependencies)
	target_link_libraries (${PROJECT_ID} PUBLIC ${ARGN})
	get_filename_component(PARENT_DIR ${CMAKE_CURRENT_SOURCE_DIR} DIRECTORY)

	foreach (ARG ${ARGN})
		target_include_directories (${PROJECT_ID} PUBLIC ${PARENT_DIR}/${ARG})
	endforeach ()
endmacro ()

macro (set_private_dependencies)
	target_link_libraries (${PROJECT_ID} PRIVATE ${ARGN})
	get_filename_component(PARENT_DIR ${CMAKE_CURRENT_SOURCE_DIR} DIRECTORY)

	foreach (ARG ${ARGN})
		target_include_directories (${PROJECT_ID} PRIVATE ${PARENT_DIR}/${ARG})
	endforeach ()
endmacro ()