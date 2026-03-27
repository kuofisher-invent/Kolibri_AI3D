require 'json'
require 'sketchup'

module Kolibri
  module SkpBridge
    extend self

    IDENTITY = Geom::Transformation.new.to_a.map { |v| v.to_f }.freeze

    class ExportObserver < Sketchup::AppObserver
      def expectsStartupModelNotifications
        true
      end

      def onNewModel(model)
        Kolibri::SkpBridge.try_export(model)
      end

      def onOpenModel(model)
        Kolibri::SkpBridge.try_export(model)
      end
    end

    def try_export(model)
      return if @export_started

      input_path = ENV['KOLIBRI_SKP_EXPORT_IN'].to_s
      output_path = ENV['KOLIBRI_SKP_EXPORT_OUT'].to_s
      return if input_path.empty? || output_path.empty?
      return unless File.exist?(input_path)

      model_path = model.path.to_s
      return if model_path.empty?
      return unless same_path?(model_path, input_path)

      @export_started = true
      UI.start_timer(0.1, false) do
        begin
          payload = export_model(model)
          File.write(output_path, JSON.pretty_generate(payload))
        rescue => e
          File.write(output_path, JSON.pretty_generate({
            'error' => "#{e.class}: #{e.message}",
            'backtrace' => e.backtrace
          }))
        ensure
          UI.start_timer(0.2, false) { Sketchup.quit }
        end
      end
    end

    def export_model(model)
      state = {
        materials: {},
        material_list: [],
        meshes: [],
        instances: [],
        groups: [],
        group_index: {},
        component_defs: [],
        component_map: {}
      }

      export_component_definitions(model, state)
      export_entities(model.entities, Geom::Transformation.new, nil, state, 'root')

      {
        'bridge_version' => '0.1.0',
        'source_file' => model.path.to_s,
        'model_name' => model.title.to_s,
        'units' => model_units(model),
        'materials' => state[:material_list],
        'meshes' => state[:meshes],
        'instances' => state[:instances],
        'groups' => state[:groups],
        'component_defs' => state[:component_defs]
      }
    end

    def export_component_definitions(model, state)
      model.definitions.each do |definition|
        next if definition.group?
        next if definition.image?

        def_id = "comp_#{entity_id(definition)}"
        mesh_ids = build_direct_meshes(
          definition.entities,
          Geom::Transformation.new,
          state,
          "def_#{entity_id(definition)}",
          definition_name(definition)
        )

        component = {
          'id' => def_id,
          'name' => definition_name(definition),
          'mesh_ids' => mesh_ids,
          'instance_count' => definition.count_instances
        }

        state[:component_map][definition] = component
        state[:component_defs] << component
      end
    end

    def export_entities(entities, transform, parent_group_id, state, prefix)
      direct_mesh_ids = build_direct_meshes(entities, transform, state, prefix, prefix)
      direct_mesh_ids.each do |mesh_id|
        create_instance(state, {
          'mesh_id' => mesh_id,
          'component_def_id' => nil,
          'transform' => IDENTITY,
          'name' => prefix,
          'layer' => '',
          'parent_group_id' => parent_group_id
        })
      end

      entities.each do |entity|
        case entity
        when Sketchup::Group
          export_group(entity, transform, parent_group_id, state)
        when Sketchup::ComponentInstance
          export_component_instance(entity, transform, parent_group_id, state)
        end
      end
    end

    def export_group(group, parent_transform, parent_group_id, state)
      gid = "grp_#{entity_id(group)}"
      create_group(state, gid, object_name(group, 'Group'), parent_group_id)

      world_transform = parent_transform * group.transformation
      export_entities(group.entities, world_transform, gid, state, gid)
    end

    def export_component_instance(instance, parent_transform, parent_group_id, state)
      definition = instance.definition
      component = state[:component_map][definition]

      gid = "cmpinst_#{entity_id(instance)}"
      create_group(state, gid, object_name(instance, definition_name(definition)), parent_group_id)

      world_transform = parent_transform * instance.transformation

      if component
        component['mesh_ids'].each do |mesh_id|
          create_instance(state, {
            'mesh_id' => mesh_id,
            'component_def_id' => component['id'],
            'transform' => world_transform.to_a.map { |v| v.to_f },
            'name' => object_name(instance, definition_name(definition)),
            'layer' => entity_layer(instance),
            'parent_group_id' => gid
          })
        end
      end

      export_nested_children(definition.entities, world_transform, gid, state)
    end

    def export_nested_children(entities, transform, parent_group_id, state)
      entities.each do |entity|
        case entity
        when Sketchup::Group
          export_group(entity, transform, parent_group_id, state)
        when Sketchup::ComponentInstance
          export_component_instance(entity, transform, parent_group_id, state)
        end
      end
    end

    def build_direct_meshes(entities, transform, state, prefix, name_base)
      buckets = {}

      entities.grep(Sketchup::Face).each do |face|
        material = face.material || face.back_material
        material_id = ensure_material(state, material)
        bucket_key = material_id || 'default'
        bucket = (buckets[bucket_key] ||= {
          'material_id' => material_id,
          'vertices' => [],
          'indices' => []
        })

        mesh = face.mesh 7
        points = mesh.points
        mesh.polygons.each do |polygon|
          point_indices = polygon.map { |idx| idx.abs - 1 }
          next if point_indices.length < 3

          transformed = point_indices.map do |point_index|
            points[point_index].transform(transform)
          end

          base = bucket['vertices'].length
          transformed.each do |pt|
            bucket['vertices'] << [pt.x.to_f, pt.y.to_f, pt.z.to_f]
          end

          (1...(transformed.length - 1)).each do |fan|
            bucket['indices'] << base
            bucket['indices'] << base + fan
            bucket['indices'] << base + fan + 1
          end
        end
      end

      mesh_ids = []
      buckets.each_with_index do |(material_key, bucket), index|
        next if bucket['indices'].empty?

        mesh_id = "#{prefix}_mesh_#{index}"
        state[:meshes] << {
          'id' => mesh_id,
          'name' => "#{name_base}_#{material_key}",
          'vertices' => bucket['vertices'],
          'normals' => [],
          'indices' => bucket['indices'],
          'material_id' => bucket['material_id']
        }
        mesh_ids << mesh_id
      end

      mesh_ids
    end

    def ensure_material(state, material)
      return nil unless material

      mat_id = "mat_#{entity_id(material)}"
      return mat_id if state[:materials].key?(mat_id)

      color = material.color
      alpha = material.alpha.to_f
      texture_path = nil
      if material.texture
        texture_path = material.texture.filename.to_s
      end

      state[:materials][mat_id] = true
      state[:material_list] << {
        'id' => mat_id,
        'name' => material.display_name.to_s,
        'color' => [
          color.red.to_f / 255.0,
          color.green.to_f / 255.0,
          color.blue.to_f / 255.0,
          alpha
        ],
        'texture_path' => texture_path,
        'opacity' => alpha
      }

      mat_id
    end

    def create_group(state, group_id, name, parent_group_id)
      return if state[:group_index].key?(group_id)

      state[:group_index][group_id] = state[:groups].length
      state[:groups] << {
        'id' => group_id,
        'name' => name,
        'children' => [],
        'parent_id' => parent_group_id
      }
    end

    def create_instance(state, data)
      instance_id = "inst_#{state[:instances].length + 1}"
      state[:instances] << {
        'id' => instance_id,
        'mesh_id' => data['mesh_id'],
        'component_def_id' => data['component_def_id'],
        'transform' => data['transform'],
        'name' => data['name'],
        'layer' => data['layer']
      }

      parent_group_id = data['parent_group_id']
      if parent_group_id && state[:group_index].key?(parent_group_id)
        index = state[:group_index][parent_group_id]
        state[:groups][index]['children'] << instance_id
      end
    end

    def entity_id(entity)
      if entity.respond_to?(:persistent_id)
        entity.persistent_id
      else
        entity.object_id
      end
    end

    def object_name(entity, fallback)
      value = entity.name.to_s.strip
      value.empty? ? fallback : value
    end

    def definition_name(definition)
      object_name(definition, 'Component')
    end

    def entity_layer(entity)
      if entity.respond_to?(:layer) && entity.layer
        entity.layer.name.to_s
      elsif entity.respond_to?(:tag) && entity.tag
        entity.tag.name.to_s
      else
        ''
      end
    end

    def model_units(model)
      unit_code = model.options['UnitsOptions']['LengthUnit']
      case unit_code
      when 0 then 'inch'
      when 1 then 'foot'
      when 2 then 'mm'
      when 3 then 'cm'
      when 4 then 'm'
      else 'inch'
      end
    rescue
      'inch'
    end

    def same_path?(a, b)
      File.expand_path(a).tr('\\', '/').downcase == File.expand_path(b).tr('\\', '/').downcase
    end

    unless @loaded
      Sketchup.add_observer(ExportObserver.new)
      @loaded = true
    end
  end
end
