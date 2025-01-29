use std::collections::{BTreeMap, HashSet};
use std::num::NonZeroU16;

use charabia::Language;
use heed::RoTxn;

use super::FieldsIdsMap;
use crate::attribute_patterns::PatternMatch;
use crate::{
    is_faceted_by, FieldId, FilterableAttributesFeatures, FilterableAttributesRule, Index,
    LocalizedAttributesRule, Result,
};

#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub searchable: bool,
    pub sortable: bool,
    localized_attributes_rule_id: Option<NonZeroU16>,
    filterable_attributes_rule_id: Option<NonZeroU16>,
}

#[derive(Debug, Clone)]
pub struct FieldIdMapWithMetadata {
    fields_ids_map: FieldsIdsMap,
    builder: MetadataBuilder,
    metadata: BTreeMap<FieldId, Metadata>,
}

impl FieldIdMapWithMetadata {
    pub fn new(existing_fields_ids_map: FieldsIdsMap, builder: MetadataBuilder) -> Self {
        let metadata = existing_fields_ids_map
            .iter()
            .map(|(id, name)| (id, builder.metadata_for_field(name)))
            .collect();
        Self { fields_ids_map: existing_fields_ids_map, builder, metadata }
    }

    pub fn as_fields_ids_map(&self) -> &FieldsIdsMap {
        &self.fields_ids_map
    }

    /// Returns the number of fields ids in the map.
    pub fn len(&self) -> usize {
        self.fields_ids_map.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.fields_ids_map.is_empty()
    }

    /// Returns the field id related to a field name, it will create a new field id if the
    /// name is not already known. Returns `None` if the maximum field id as been reached.
    pub fn insert(&mut self, name: &str) -> Option<FieldId> {
        let id = self.fields_ids_map.insert(name)?;
        self.metadata.insert(id, self.builder.metadata_for_field(name));
        Some(id)
    }

    /// Get the id of a field based on its name.
    pub fn id(&self, name: &str) -> Option<FieldId> {
        self.fields_ids_map.id(name)
    }

    pub fn id_with_metadata(&self, name: &str) -> Option<(FieldId, Metadata)> {
        let id = self.fields_ids_map.id(name)?;
        Some((id, self.metadata(id).unwrap()))
    }

    /// Get the name of a field based on its id.
    pub fn name(&self, id: FieldId) -> Option<&str> {
        self.fields_ids_map.name(id)
    }

    /// Get the name of a field based on its id.
    pub fn name_with_metadata(&self, id: FieldId) -> Option<(&str, Metadata)> {
        let name = self.fields_ids_map.name(id)?;
        Some((name, self.metadata(id).unwrap()))
    }

    pub fn metadata(&self, id: FieldId) -> Option<Metadata> {
        self.metadata.get(&id).copied()
    }

    /// Iterate over the ids and names in the ids order.
    pub fn iter(&self) -> impl Iterator<Item = (FieldId, &str, Metadata)> {
        self.fields_ids_map.iter().map(|(id, name)| (id, name, self.metadata(id).unwrap()))
    }

    pub fn iter_id_metadata(&self) -> impl Iterator<Item = (FieldId, Metadata)> + '_ {
        self.metadata.iter().map(|(k, v)| (*k, *v))
    }

    pub fn iter_metadata(&self) -> impl Iterator<Item = Metadata> + '_ {
        self.metadata.values().copied()
    }

    pub fn metadata_builder(&self) -> &MetadataBuilder {
        &self.builder
    }
}

impl Metadata {
    pub fn locales<'rules>(
        &self,
        rules: &'rules [LocalizedAttributesRule],
    ) -> Option<&'rules [Language]> {
        let localized_attributes_rule_id = self.localized_attributes_rule_id?.get();
        // - 1: `localized_attributes_rule_id` is NonZero
        let rule = rules.get((localized_attributes_rule_id - 1) as usize).unwrap();
        Some(rule.locales())
    }

    pub fn filterable_attributes<'rules>(
        &self,
        rules: &'rules [FilterableAttributesRule],
    ) -> Option<&'rules FilterableAttributesRule> {
        let filterable_attributes_rule_id = self.filterable_attributes_rule_id?.get();
        // - 1: `filterable_attributes_rule_id` is NonZero
        let rule = rules.get((filterable_attributes_rule_id - 1) as usize).unwrap();
        Some(rule)
    }

    pub fn filterable_attributes_features(
        &self,
        rules: &[FilterableAttributesRule],
    ) -> FilterableAttributesFeatures {
        self.filterable_attributes(rules)
            .map(|rule| rule.features())
            // if there is no filterable attributes rule, return no features
            .unwrap_or_else(FilterableAttributesFeatures::no_features)
    }

    pub fn is_sortable(&self) -> bool {
        self.sortable
    }

    pub fn is_searchable(&self) -> bool {
        self.searchable
    }

    /// Returns `true` if the field is part of the facet databases. (sortable, filterable, or facet searchable)
    pub fn is_faceted(&self, rules: &[FilterableAttributesRule]) -> bool {
        if self.is_sortable() {
            return true;
        }

        let features = self.filterable_attributes_features(&rules);
        if features.is_filterable() {
            return true;
        }

        if features.is_facet_searchable() {
            return true;
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct MetadataBuilder {
    searchable_attributes: Option<Vec<String>>,
    filterable_attributes: Vec<FilterableAttributesRule>,
    sortable_attributes: HashSet<String>,
    localized_attributes: Option<Vec<LocalizedAttributesRule>>,
}

impl MetadataBuilder {
    pub fn from_index(index: &Index, rtxn: &RoTxn) -> Result<Self> {
        let searchable_attributes = match index.user_defined_searchable_fields(rtxn)? {
            Some(fields) if fields.contains(&"*") => None,
            None => None,
            Some(fields) => Some(fields.into_iter().map(|s| s.to_string()).collect()),
        };
        let filterable_attributes = index.filterable_attributes_rules(rtxn)?;
        let sortable_attributes = index.sortable_fields(rtxn)?;
        let localized_attributes = index.localized_attributes_rules(rtxn)?;

        Ok(Self {
            searchable_attributes,
            filterable_attributes,
            sortable_attributes,
            localized_attributes,
        })
    }

    // pub fn new(
    //     searchable_attributes: Option<Vec<String>>,
    //     filterable_attributes: Vec<FilterableAttributesRule>,
    //     sortable_attributes: HashSet<String>,
    //     localized_attributes: Option<Vec<LocalizedAttributesRule>>,
    // ) -> Self {
    //     let searchable_attributes = match searchable_attributes {
    //         Some(fields) if fields.iter().any(|f| f == "*") => None,
    //         None => None,
    //         Some(fields) => Some(fields),
    //     };

    //     Self {
    //         searchable_attributes,
    //         filterable_attributes,
    //         sortable_attributes,
    //         localized_attributes,
    //     }
    // }

    pub fn metadata_for_field(&self, field: &str) -> Metadata {
        let searchable = match &self.searchable_attributes {
            // A field is searchable if it is faceted by a searchable attribute
            Some(attributes) => attributes.iter().any(|pattern| is_faceted_by(field, pattern)),
            None => true,
        };

        // A field is sortable if it is faceted by a sortable attribute
        let sortable = self.sortable_attributes.iter().any(|pattern| is_faceted_by(field, pattern));

        let localized_attributes_rule_id = self
            .localized_attributes
            .iter()
            .flat_map(|v| v.iter())
            .position(|rule| rule.match_str(field) == PatternMatch::Match)
            // saturating_add(1): make `id` `NonZero`
            .map(|id| NonZeroU16::new(id.saturating_add(1).try_into().unwrap()).unwrap());

        let filterable_attributes_rule_id = self
            .filterable_attributes
            .iter()
            .position(|attribute| attribute.match_str(field) == PatternMatch::Match)
            // saturating_add(1): make `id` `NonZero`
            .map(|id| NonZeroU16::new(id.saturating_add(1).try_into().unwrap()).unwrap());

        Metadata {
            searchable,
            sortable,
            localized_attributes_rule_id,
            filterable_attributes_rule_id,
        }
    }

    pub fn searchable_attributes(&self) -> Option<&[String]> {
        self.searchable_attributes.as_deref()
    }

    pub fn sortable_attributes(&self) -> &HashSet<String> {
        &self.sortable_attributes
    }

    pub fn filterable_attributes(&self) -> &[FilterableAttributesRule] {
        &self.filterable_attributes
    }

    pub fn localized_attributes_rules(&self) -> Option<&[LocalizedAttributesRule]> {
        self.localized_attributes.as_deref()
    }
}
