{% macro test() %}
{{ other_other_macro() }}
{% endmacro %}

{{ ref(other_other_macro(ref('my_first_dbt_model'))) }}