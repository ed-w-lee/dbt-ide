{% macro test() %}
{{ other_other_macro() }}
{% endmacro %}

{% materialization something, adapter="another", adapter="uwu" %}
something
{% endmaterialization %}

{{ ref('my_first_dbt_model')}}